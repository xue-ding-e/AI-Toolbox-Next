//! One bounded, typed request per same-user connection. There is no TCP,
//! bearer token, credential payload, generic JSON invoke, or automatic server.
//!
//! Runtime precondition: bind and all async operations must run within a Tokio
//! runtime with I/O and time enabled for their entire use. A current handle alone
//! does not prove those drivers are enabled. Desktop integration must verify this
//! before exposing the endpoint; disabled drivers can panic inside Tokio.

use serde::{Deserialize, Serialize};

use crate::owner::{OwnerEpoch, OwnerPermit, ProfileIdentity, ProfileKey, RuntimeDirectory};
use crate::{Error, Result};

pub const PROTOCOL_MAJOR: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 16 * 1024;
pub const IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
pub const ENDPOINT_PREFIX: &str = ".ai-toolbox-owner-";
pub const ENDPOINT_SUFFIX: &str = ".sock";

pub fn endpoint_basename(profile: ProfileKey) -> String {
    format!("{ENDPOINT_PREFIX}{:016x}-{:016x}{ENDPOINT_SUFFIX}", profile.device, profile.inode)
}

/// Match reserved coordination path segments exactly, including segments of
/// archive members. Do not use a broad dotfile or suffix filter instead.
pub fn is_reserved_coordination_name(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else { return false; };
    if name == crate::owner::PROFILE_LEASE_FILE || name == crate::owner::RESOURCE_LEASE_FILE {
        return true;
    }
    let Some(identity) = name.strip_prefix(ENDPOINT_PREFIX).and_then(|name| name.strip_suffix(ENDPOINT_SUFFIX)) else { return false; };
    let bytes = identity.as_bytes();
    bytes.len() == 33 && bytes[16] == b'-' && bytes.iter().enumerate().all(|(index, byte)| {
        index == 16 || byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)
    })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    protocol_major: u16,
    request_id: u64,
    profile: ProfileKey,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HelloAck {
    protocol_major: u16,
    request_id: u64,
    profile: ProfileKey,
    owner_epoch: OwnerEpoch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerOperation {
    OwnerStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerRequest {
    pub protocol_major: u16,
    pub request_id: u64,
    pub profile: ProfileKey,
    pub owner_epoch: OwnerEpoch,
    pub operation: OwnerOperation,
}

/// This reports only a held profile lease. It does not claim desktop writer
/// coverage, database readiness, runtime projection, or any business capability.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerStatus {
    pub profile_lease_held: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerResponse {
    protocol_major: u16,
    request_id: u64,
    profile: ProfileKey,
    owner_epoch: OwnerEpoch,
    status: OwnerStatus,
}

fn validate_version(version: u16) -> Result<()> {
    if version == PROTOCOL_MAJOR { Ok(()) } else { Err(Error::ProtocolMismatch) }
}

fn validate_request_id(request_id: u64) -> Result<()> {
    if request_id != 0 { Ok(()) } else { Err(Error::InvalidMessage) }
}

fn validate_peer_uid(peer_uid: u32, current_uid: u32) -> Result<()> {
    if peer_uid == current_uid { Ok(()) } else { Err(Error::PeerRejected) }
}

#[cfg(target_os = "linux")]
pub use linux::{OwnerClient, OwnerServer, PendingRequest};

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde::de::DeserializeOwned;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::time::timeout;

    use super::*;
    use crate::owner::linux::{current_uid, key};

    fn endpoint_path(profile: &ProfileIdentity, runtime: &RuntimeDirectory) -> Result<PathBuf> {
        profile.ensure_current()?;
        runtime.ensure_current()?;
        let path = runtime.path().join(endpoint_basename(profile.key()));
        // Linux sockaddr_un.sun_path is 108 bytes, including the trailing NUL.
        if path.as_os_str().as_bytes().len() >= 108 {
            return Err(Error::UnsafeDirectory);
        }
        Ok(path)
    }

    fn check_socket(path: &std::path::Path) -> Result<ProfileKey> {
        let meta = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound { Error::OwnerUnavailable } else { Error::Io(error) }
        })?;
        if !meta.file_type().is_socket() || meta.uid() != current_uid()? || meta.nlink() != 1 {
            return Err(Error::EndpointConflict);
        }
        Ok(key(&meta))
    }

    fn check_peer(stream: &UnixStream) -> Result<()> {
        // On Linux Tokio reads SO_PEERCRED. Neither process IDs nor the
        // unauthenticated handshake are used as authority for peer identity.
        validate_peer_uid(stream.peer_cred()?.uid(), current_uid()?)
    }

    async fn read_frame<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T> {
        let mut header = [0; 4];
        stream.read_exact(&mut header).await?;
        let length = u32::from_be_bytes(header) as usize;
        if length > MAX_FRAME_BYTES { return Err(Error::FrameTooLarge); }
        if length == 0 { return Err(Error::InvalidMessage); }
        let mut bytes = vec![0; length];
        stream.read_exact(&mut bytes).await?;
        // Do not reflect untrusted input or serde excerpts into diagnostics.
        serde_json::from_slice(&bytes).map_err(|_| Error::InvalidMessage)
    }

    async fn write_frame<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|_| Error::InvalidMessage)?;
        if bytes.len() > MAX_FRAME_BYTES { return Err(Error::FrameTooLarge); }
        stream.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
        stream.write_all(&bytes).await?;
        stream.flush().await?;
        Ok(())
    }

    struct BoundEndpoint {
        owner: OwnerPermit,
        runtime: RuntimeDirectory,
        path: PathBuf,
        socket_key: ProfileKey,
    }

    impl BoundEndpoint {
        fn ensure_current(&self) -> Result<()> {
            self.owner.ensure_current()?;
            self.runtime.ensure_current()?;
            if check_socket(&self.path)? != self.socket_key {
                return Err(Error::EndpointConflict);
            }
            Ok(())
        }
    }

    impl Drop for BoundEndpoint {
        fn drop(&mut self) {
            // Only our created socket, still in our private directory and still
            // protected by our live profile lease, is eligible for removal.
            // Do not remove unknown/stale endpoints or any lock file.
            if self.ensure_current().is_ok() {
                let _ = fs::remove_file(&self.path);
            }
        }
    }

    pub struct OwnerServer {
        // Close the listener before releasing its endpoint/owner capability.
        listener: UnixListener,
        endpoint: Arc<BoundEndpoint>,
    }

    impl OwnerServer {
        /// Explicit bind only. The caller must already own the profile and have
        /// audited every writer before exposing this boundary in the desktop.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub fn bind(owner: OwnerPermit, runtime: RuntimeDirectory) -> Result<Self> {
            tokio::runtime::Handle::try_current().map_err(|_| Error::RuntimeUnavailable)?;
            owner.ensure_current()?;
            let path = endpoint_path(owner.profile(), &runtime)?;
            match fs::symlink_metadata(&path) {
                Ok(_) => return Err(Error::EndpointConflict),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                Err(error) => return Err(error.into()),
            }
            // No stale-socket unlink: a crash currently requires explicit,
            // separately verified recovery. Existence is never a PID test.
            let listener = UnixListener::bind(&path).map_err(|error| {
                if error.kind() == io::ErrorKind::AddrInUse { Error::EndpointConflict } else { Error::Io(error) }
            })?;
            let socket_key = check_socket(&path)?;
            let endpoint = Arc::new(BoundEndpoint { owner, runtime, path, socket_key });
            endpoint.ensure_current()?;
            Ok(Self { listener, endpoint })
        }

        /// Bounded accept + peer proof + handshake + one complete typed request.
        /// No background task or arbitrary dispatch is started by this crate.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn accept(&self) -> Result<PendingRequest> {
            self.endpoint.ensure_current()?;
            timeout(IO_TIMEOUT, async {
                let (mut stream, _) = self.listener.accept().await?;
                check_peer(&stream)?;
                self.endpoint.ensure_current()?;
                let hello: Hello = read_frame(&mut stream).await?;
                validate_version(hello.protocol_major)?;
                validate_request_id(hello.request_id)?;
                let owner = &self.endpoint.owner;
                if hello.profile != owner.profile().key() { return Err(Error::StaleOwner); }
                write_frame(&mut stream, &HelloAck {
                    protocol_major: PROTOCOL_MAJOR,
                    request_id: hello.request_id,
                    profile: owner.profile().key(),
                    owner_epoch: owner.epoch(),
                }).await?;
                let request: OwnerRequest = read_frame(&mut stream).await?;
                validate_version(request.protocol_major)?;
                if request.request_id != hello.request_id { return Err(Error::InvalidMessage); }
                if request.profile != owner.profile().key() || request.owner_epoch != owner.epoch() {
                    return Err(Error::StaleOwner);
                }
                self.endpoint.ensure_current()?;
                Ok(PendingRequest { stream, request, endpoint: self.endpoint.clone() })
            }).await.map_err(|_| Error::TimedOut)?
        }
    }

    /// Retains the endpoint and owner even if the listening task is stopped.
    /// Dropping/cancelling a request closes its connection, never its shared lease.
    pub struct PendingRequest {
        stream: UnixStream,
        request: OwnerRequest,
        endpoint: Arc<BoundEndpoint>,
    }

    impl PendingRequest {
        pub fn request(&self) -> &OwnerRequest { &self.request }
        pub fn owner(&self) -> &OwnerPermit { &self.endpoint.owner }

        /// Consume the only supported request. There is no raw response writer
        /// with which a caller could expose unreviewed business payloads.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn respond_status(mut self) -> Result<()> {
            self.endpoint.ensure_current()?;
            let response = OwnerResponse {
                protocol_major: PROTOCOL_MAJOR,
                request_id: self.request.request_id,
                profile: self.endpoint.owner.profile().key(),
                owner_epoch: self.endpoint.owner.epoch(),
                status: OwnerStatus { profile_lease_held: true },
            };
            timeout(IO_TIMEOUT, write_frame(&mut self.stream, &response)).await.map_err(|_| Error::TimedOut)?
        }
    }

    pub struct OwnerClient {
        stream: UnixStream,
        profile: ProfileIdentity,
        runtime: RuntimeDirectory,
        path: PathBuf,
        socket_key: ProfileKey,
        request_id: u64,
        epoch: OwnerEpoch,
    }

    impl OwnerClient {
        /// Pure client discovery: no profile/lock/runtime creation and no DB open.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn connect(profile: &ProfileIdentity, runtime: &RuntimeDirectory, request_id: u64) -> Result<Self> {
            tokio::runtime::Handle::try_current().map_err(|_| Error::RuntimeUnavailable)?;
            validate_request_id(request_id)?;
            let path = endpoint_path(profile, runtime)?;
            let socket_key = check_socket(&path)?;
            timeout(IO_TIMEOUT, async {
                let mut stream = UnixStream::connect(&path).await.map_err(|error| {
                    if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) {
                        Error::OwnerUnavailable
                    } else { Error::Io(error) }
                })?;
                check_peer(&stream)?;
                runtime.ensure_current()?;
                if check_socket(&path)? != socket_key { return Err(Error::EndpointConflict); }
                write_frame(&mut stream, &Hello { protocol_major: PROTOCOL_MAJOR, request_id, profile: profile.key() }).await?;
                let ack: HelloAck = read_frame(&mut stream).await?;
                validate_version(ack.protocol_major)?;
                if ack.request_id != request_id { return Err(Error::InvalidMessage); }
                if ack.profile != profile.key() { return Err(Error::StaleOwner); }
                profile.ensure_current()?;
                Ok(Self { stream, profile: profile.clone(), runtime: runtime.clone(), path, socket_key, request_id, epoch: ack.owner_epoch })
            }).await.map_err(|_| Error::TimedOut)?
        }

        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn owner_status(mut self) -> Result<OwnerStatus> {
            self.profile.ensure_current()?;
            self.runtime.ensure_current()?;
            if check_socket(&self.path)? != self.socket_key { return Err(Error::EndpointConflict); }
            timeout(IO_TIMEOUT, async {
                write_frame(&mut self.stream, &OwnerRequest {
                    protocol_major: PROTOCOL_MAJOR,
                    request_id: self.request_id,
                    profile: self.profile.key(),
                    owner_epoch: self.epoch,
                    operation: OwnerOperation::OwnerStatus,
                }).await?;
                let response: OwnerResponse = read_frame(&mut self.stream).await?;
                validate_version(response.protocol_major)?;
                if response.request_id != self.request_id { return Err(Error::InvalidMessage); }
                if response.profile != self.profile.key() || response.owner_epoch != self.epoch {
                    return Err(Error::StaleOwner);
                }
                if !response.status.profile_lease_held { return Err(Error::InvalidMessage); }
                Ok(response.status)
            }).await.map_err(|_| Error::TimedOut)?
        }
    }
}

// Other platforms compile with an explicit failure instead of pretending a
// Unix path supplies Windows named-pipe ACL/peer validation or macOS proofs.
#[cfg(not(target_os = "linux"))]
pub struct OwnerServer;
#[cfg(not(target_os = "linux"))]
impl OwnerServer {
    pub fn bind(_owner: OwnerPermit, _runtime: RuntimeDirectory) -> Result<Self> { Err(Error::UnsupportedPlatform) }
    pub async fn accept(&self) -> Result<PendingRequest> { Err(Error::UnsupportedPlatform) }
}
#[cfg(not(target_os = "linux"))]
pub struct PendingRequest;
#[cfg(not(target_os = "linux"))]
impl PendingRequest {
    pub async fn respond_status(self) -> Result<()> { Err(Error::UnsupportedPlatform) }
}
#[cfg(not(target_os = "linux"))]
pub struct OwnerClient;
#[cfg(not(target_os = "linux"))]
impl OwnerClient {
    pub async fn connect(_profile: &ProfileIdentity, _runtime: &RuntimeDirectory, _request_id: u64) -> Result<Self> { Err(Error::UnsupportedPlatform) }
    pub async fn owner_status(self) -> Result<OwnerStatus> { Err(Error::UnsupportedPlatform) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_names_are_exact_segments_not_a_dotfile_filter() {
        for name in [
            crate::owner::PROFILE_LEASE_FILE.to_owned(),
            crate::owner::RESOURCE_LEASE_FILE.to_owned(),
            endpoint_basename(ProfileKey { device: 1, inode: u64::MAX }),
        ] {
            assert!(is_reserved_coordination_name(std::ffi::OsStr::new(&name)));
        }
        for name in [
            ".git", ".codex", "config.toml", ".ai-toolbox-owner.lock.backup",
            ".ai-toolbox-owner-1-2.sock",
            ".ai-toolbox-owner-000000000000000g-0000000000000001.sock",
            "folder/.ai-toolbox-owner.lock",
        ] {
            assert!(!is_reserved_coordination_name(std::ffi::OsStr::new(name)));
        }
    }

    #[test]
    fn handshake_and_operation_reject_unreviewed_input() {
        assert!(serde_json::from_str::<Hello>(
            r#"{"protocol_major":1,"request_id":1,"profile":{"device":1,"inode":1},"invoke":"shell"}"#
        ).is_err());
        assert!(serde_json::from_str::<OwnerOperation>(r#""invoke""#).is_err());
        assert!(matches!(validate_version(0), Err(Error::ProtocolMismatch)));
        assert!(matches!(validate_request_id(0), Err(Error::InvalidMessage)));
    }

    #[test]
    fn peer_uid_comparison_only_is_not_a_native_cross_user_proof() {
        assert!(validate_peer_uid(1000, 1000).is_ok());
        assert!(matches!(validate_peer_uid(1001, 1000), Err(Error::PeerRejected)));
        assert!(matches!(validate_peer_uid(0, 1000), Err(Error::PeerRejected)));
    }
}
