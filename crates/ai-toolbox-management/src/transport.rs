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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum OwnerOperation {
    OwnerStatus,
    OwnerCapabilities,
    ContextInspect,
    ContextSet(ContextSetRequest),
    ContextRequestStatus(ContextRequestStatusRequest),
}

/// Explicitly advertised by the installed owner session, never inferred from
/// a build feature, a successful handshake, or the existing owner-status bit.
/// This observation is not a writer permit; execution still rechecks admission.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerCapability {
    StatusOnly,
    ContextSetReady,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextTarget {
    Common,
    Provider,
}

/// Only these two numeric overrides may cross the context write boundary.
/// None means unchanged, not removal; a patch must contain at least one value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextPatch {
    pub model_context_window: Option<u64>,
    pub model_auto_compact_token_limit: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSetRequest {
    pub schema_version: u32,
    /// Persistent idempotency ID, distinct from the connection's numeric ID.
    pub request_id: String,
    pub expected_provider_id: String,
    pub expected_revision: String,
    pub target: ContextTarget,
    pub patch: ContextPatch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRequestStatusRequest {
    pub request_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    Common,
    Provider,
    Unspecified,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextOverride {
    pub effective: Option<u64>,
    pub source: ContextSource,
}

/// Merged master overrides, not model defaults or live-file verification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextInspection {
    pub provider_id: String,
    pub revision: String,
    pub model_context_window: ContextOverride,
    pub model_auto_compact_token_limit: ContextOverride,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPhase {
    SavedOnly,
    ProjectionStarted,
    ProjectedUnverified,
    Verified,
    ProjectionUncertain,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextFailure {
    ProjectionPreflightFailed,
    ProjectionOutcomeUnknown,
    ReadbackFailed,
    ReadbackMismatch,
}

/// Historical whitelisted metadata only, never a serialized journal record.
/// Only Verified denotes success; receiving any other phase does not authorize
/// retry, rollback, recovery, or another file effect.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextOutcome {
    pub request_id: String,
    pub provider_id: String,
    pub phase: ContextPhase,
    pub failure: Option<ContextFailure>,
    pub replay: bool,
    pub model_context_window: Option<u64>,
    pub model_auto_compact_token_limit: Option<u64>,
}

/// Static diagnostics only. No arbitrary error text, configuration, paths,
/// credentials, database records, or caller-provided JSON is reflected here.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextErrorCode {
    Unsupported,
    InvalidRequest,
    RequestNotFound,
    Ownership,
    Storage,
    InvalidMaster,
    ProviderChanged,
    RevisionConflict,
    RuntimeUnsupported,
    FileUnavailable,
    JournalConflict,
    JournalInvalid,
    PersistenceUncertain,
    PhasePersistence,
    Interrupted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, thiserror::Error)]
#[error("Context operation error: {code:?}")]
#[serde(deny_unknown_fields)]
pub struct ContextError {
    pub code: ContextErrorCode,
    pub partial: Option<ContextOutcome>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextResponse {
    Inspect(ContextInspection),
    Outcome(ContextOutcome),
    Error(ContextError),
}

/// Client-local errors are not wire payloads. Once a set frame starts writing,
/// a missing or invalid response cannot establish whether effects occurred.
#[derive(Debug, thiserror::Error)]
pub enum ContextClientError {
    #[error("Context owner transport failed before a confirmed response")]
    Owner(#[from] Error),
    /// A non-success response, not a claim that no partial effects occurred.
    #[error("Context operation returned an error; partial effects may exist")]
    Rejected(#[source] ContextError),
    #[error("Context set outcome is unknown; do not automatically retry")]
    OutcomeUnknown,
}

pub type ContextResult<T> = std::result::Result<T, ContextClientError>;

fn validate_context_id(value: &str) -> Result<()> {
    if !value.is_empty() && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    { Ok(()) } else { Err(Error::InvalidMessage) }
}

fn validate_context_revision(value: &str) -> Result<()> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    { Ok(()) } else { Err(Error::InvalidMessage) }
}

fn validate_context_values(context: Option<u64>, compact: Option<u64>) -> Result<()> {
    if context.into_iter().chain(compact).any(|value| value == 0 || value > i64::MAX as u64)
        || matches!((context, compact), (Some(context), Some(compact)) if compact > context)
    { Err(Error::InvalidMessage) } else { Ok(()) }
}

impl ContextRequestStatusRequest {
    pub fn validate(&self) -> Result<()> {
        validate_context_id(&self.request_id)
    }
}

impl ContextSetRequest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1
            || (self.patch.model_context_window.is_none() && self.patch.model_auto_compact_token_limit.is_none())
        { return Err(Error::InvalidMessage); }
        validate_context_id(&self.request_id)?;
        validate_context_id(&self.expected_provider_id)?;
        validate_context_revision(&self.expected_revision)?;
        validate_context_values(self.patch.model_context_window, self.patch.model_auto_compact_token_limit)
    }
}

impl OwnerOperation {
    fn validate(&self) -> Result<()> {
        match self {
            Self::OwnerStatus | Self::OwnerCapabilities | Self::ContextInspect => Ok(()),
            Self::ContextSet(request) => request.validate(),
            Self::ContextRequestStatus(request) => request.validate(),
        }
    }
}

impl ContextInspection {
    fn validate(&self) -> Result<()> {
        validate_context_id(&self.provider_id)?;
        validate_context_revision(&self.revision)?;
        for value in [&self.model_context_window, &self.model_auto_compact_token_limit] {
            if value.effective.is_none() != (value.source == ContextSource::Unspecified) {
                return Err(Error::InvalidMessage);
            }
        }
        validate_context_values(self.model_context_window.effective, self.model_auto_compact_token_limit.effective)
    }
}

impl ContextOutcome {
    fn validate(&self) -> Result<()> {
        validate_context_id(&self.request_id)?;
        validate_context_id(&self.provider_id)?;
        if self.model_context_window.is_none() && self.model_auto_compact_token_limit.is_none() {
            return Err(Error::InvalidMessage);
        }
        validate_context_values(self.model_context_window, self.model_auto_compact_token_limit)?;
        if self.replay && self.phase == ContextPhase::ProjectionStarted {
            return Err(Error::InvalidMessage);
        }
        match (self.phase, self.failure) {
            (ContextPhase::SavedOnly, None | Some(ContextFailure::ProjectionPreflightFailed | ContextFailure::ReadbackFailed | ContextFailure::ReadbackMismatch))
            | (ContextPhase::ProjectionStarted, None)
            | (ContextPhase::ProjectedUnverified, None | Some(ContextFailure::ReadbackFailed | ContextFailure::ReadbackMismatch))
            | (ContextPhase::Verified, None)
            | (ContextPhase::ProjectionUncertain, Some(ContextFailure::ProjectionOutcomeUnknown)) => Ok(()),
            // A replay of a persisted start can be uncertain without a persisted failure.
            (ContextPhase::ProjectionUncertain, None) if self.replay => Ok(()),
            _ => Err(Error::InvalidMessage),
        }
    }

    fn validate_binding(&self, operation: &OwnerOperation) -> Result<()> {
        self.validate()?;
        match operation {
            OwnerOperation::ContextSet(request)
                if self.request_id == request.request_id && self.provider_id == request.expected_provider_id => Ok(()),
            // Historical provider identity comes from this request, not today's provider.
            OwnerOperation::ContextRequestStatus(request) if self.request_id == request.request_id => Ok(()),
            _ => Err(Error::InvalidMessage),
        }
    }
}

impl ContextResponse {
    fn validate_for(&self, operation: &OwnerOperation) -> Result<()> {
        operation.validate()?;
        match (operation, self) {
            (OwnerOperation::ContextInspect, Self::Inspect(inspection)) => inspection.validate(),
            (OwnerOperation::ContextSet(_) | OwnerOperation::ContextRequestStatus(_), Self::Outcome(outcome)) => outcome.validate_binding(operation),
            (OwnerOperation::OwnerStatus | OwnerOperation::OwnerCapabilities, _) => Err(Error::InvalidMessage),
            (_, Self::Error(error)) => {
                if let Some(partial) = &error.partial {
                    if partial.phase == ContextPhase::Verified { return Err(Error::InvalidMessage); }
                    partial.validate_binding(operation)?;
                }
                Ok(())
            }
            _ => Err(Error::InvalidMessage),
        }
    }
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

// Capability discovery is a separate exact operation and envelope so older
// owner-status clients and servers keep their original v1 representation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerCapabilitiesResponse {
    protocol_major: u16,
    request_id: u64,
    profile: ProfileKey,
    owner_epoch: OwnerEpoch,
    capability: OwnerCapability,
}

// Separate envelope: adding fields to OwnerResponse would break existing
// deny_unknown_fields clients even though their status operation is unchanged.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ContextEnvelope {
    protocol_major: u16,
    request_id: u64,
    profile: ProfileKey,
    owner_epoch: OwnerEpoch,
    response: ContextResponse,
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

    fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>> {
        let bytes = serde_json::to_vec(value).map_err(|_| Error::InvalidMessage)?;
        if bytes.len() > MAX_FRAME_BYTES { return Err(Error::FrameTooLarge); }
        Ok(bytes)
    }

    async fn write_encoded_frame(stream: &mut UnixStream, bytes: &[u8]) -> Result<()> {
        stream.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
        stream.write_all(bytes).await?;
        stream.flush().await?;
        Ok(())
    }

    async fn write_frame<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<()> {
        write_encoded_frame(stream, &encode_frame(value)?).await
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
                request.operation.validate()?;
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

        /// Recheck the authenticated endpoint and session at actual execution,
        /// not merely when a request was accepted or queued. This is not a
        /// writer permit: the private service acquires its non-reentrant gate.
        pub(crate) fn require_context_owner(&self, owner: &OwnerPermit) -> Result<()> {
            self.endpoint.ensure_current()?;
            owner.ensure_current()?;
            let endpoint_owner = &self.endpoint.owner;
            if self.request.profile != owner.profile().key()
                || self.request.owner_epoch != owner.epoch()
                || endpoint_owner.profile().key() != owner.profile().key()
                || endpoint_owner.epoch() != owner.epoch()
            {
                return Err(Error::StaleOwner);
            }
            Ok(())
        }

        /// Consume only an owner-status request using the unchanged v1 wire.
        /// There is no raw response writer for unreviewed business payloads.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn respond_status(mut self) -> Result<()> {
            if self.request.operation != OwnerOperation::OwnerStatus {
                return Err(Error::InvalidMessage);
            }
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

        /// The owning GUI session must select this value from its actual
        /// installed facade/admission state, not just a compile-time feature.
        /// This transport does not install, enable, or dispatch that capability.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn respond_capabilities(mut self, capability: OwnerCapability) -> Result<()> {
            if self.request.operation != OwnerOperation::OwnerCapabilities {
                return Err(Error::InvalidMessage);
            }
            self.endpoint.ensure_current()?;
            let response = OwnerCapabilitiesResponse {
                protocol_major: PROTOCOL_MAJOR,
                request_id: self.request.request_id,
                profile: self.endpoint.owner.profile().key(),
                owner_epoch: self.endpoint.owner.epoch(),
                capability,
            };
            timeout(IO_TIMEOUT, write_frame(&mut self.stream, &response)).await.map_err(|_| Error::TimedOut)?
        }

        /// Only the closed context whitelist can be returned, and its kind and
        /// persistent identity must match the operation actually received.
        /// This method does not dispatch business work or grant write authority.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn respond_context(mut self, response: ContextResponse) -> Result<()> {
            response.validate_for(&self.request.operation)?;
            self.endpoint.ensure_current()?;
            let envelope = ContextEnvelope {
                protocol_major: PROTOCOL_MAJOR,
                request_id: self.request.request_id,
                profile: self.endpoint.owner.profile().key(),
                owner_epoch: self.endpoint.owner.epoch(),
                response,
            };
            timeout(IO_TIMEOUT, write_frame(&mut self.stream, &envelope)).await.map_err(|_| Error::TimedOut)?
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

        /// One read-only discovery request per connection. Unknown capabilities,
        /// legacy responses, and connection errors remain errors, never ready.
        /// A subsequent operation needs a new connection and its own admission.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn owner_capabilities(mut self) -> Result<OwnerCapability> {
            self.ensure_current()?;
            timeout(IO_TIMEOUT, async {
                write_frame(&mut self.stream, &OwnerRequest {
                    protocol_major: PROTOCOL_MAJOR,
                    request_id: self.request_id,
                    profile: self.profile.key(),
                    owner_epoch: self.epoch,
                    operation: OwnerOperation::OwnerCapabilities,
                }).await?;
                let response: OwnerCapabilitiesResponse = read_frame(&mut self.stream).await?;
                validate_version(response.protocol_major)?;
                if response.request_id != self.request_id { return Err(Error::InvalidMessage); }
                if response.profile != self.profile.key() || response.owner_epoch != self.epoch {
                    return Err(Error::StaleOwner);
                }
                self.profile.ensure_current()?;
                self.runtime.ensure_current()?;
                Ok(response.capability)
            }).await.map_err(|_| Error::TimedOut)?
        }

        fn ensure_current(&self) -> Result<()> {
            self.profile.ensure_current()?;
            self.runtime.ensure_current()?;
            if check_socket(&self.path)? != self.socket_key { return Err(Error::EndpointConflict); }
            Ok(())
        }

        async fn context_exchange(mut self, operation: OwnerOperation) -> ContextResult<ContextResponse> {
            operation.validate()?;
            self.ensure_current()?;
            let is_set = matches!(&operation, OwnerOperation::ContextSet(_));
            let request = OwnerRequest {
                protocol_major: PROTOCOL_MAJOR,
                request_id: self.request_id,
                profile: self.profile.key(),
                owner_epoch: self.epoch,
                operation,
            };
            // Validation/serialization/size failures here precede all writing.
            let bytes = encode_frame(&request)?;
            let mut write_started = false;
            let result = timeout(IO_TIMEOUT, async {
                self.ensure_current()?;
                // Conservative boundary includes partial header/body writes and
                // a failure of the very first write. Never retry this request.
                write_started = true;
                write_encoded_frame(&mut self.stream, &bytes).await?;
                let envelope: ContextEnvelope = read_frame(&mut self.stream).await?;
                validate_version(envelope.protocol_major)?;
                if envelope.request_id != self.request_id { return Err(Error::InvalidMessage); }
                if envelope.profile != self.profile.key() || envelope.owner_epoch != self.epoch {
                    return Err(Error::StaleOwner);
                }
                // The owner may close its listening endpoint immediately after
                // replying. That must not erase a valid result on this already
                // authenticated connection; the envelope still binds its epoch.
                self.profile.ensure_current()?;
                self.runtime.ensure_current()?;
                envelope.response.validate_for(&request.operation)?;
                Ok(envelope.response)
            }).await.map_err(|_| Error::TimedOut).and_then(|result| result);
            match result {
                Ok(response) => Ok(response),
                Err(_) if is_set && write_started => Err(ContextClientError::OutcomeUnknown),
                Err(error) => Err(ContextClientError::Owner(error)),
            }
        }

        /// Read whitelisted merged overrides; does not prove live-file state.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn context_inspect(self) -> ContextResult<ContextInspection> {
            match self.context_exchange(OwnerOperation::ContextInspect).await? {
                ContextResponse::Inspect(inspection) => Ok(inspection),
                ContextResponse::Error(error) => Err(ContextClientError::Rejected(error)),
                _ => Err(Error::InvalidMessage.into()),
            }
        }

        /// A returned outcome is successful only in the Verified phase.
        /// Dropping/cancelling this future after its first poll may also leave an
        /// unknown outcome. Disconnect is never cancellation or rollback; only
        /// an explicit status query can obtain the recorded result, if any.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn context_set(self, request: ContextSetRequest) -> ContextResult<ContextOutcome> {
            match self.context_exchange(OwnerOperation::ContextSet(request)).await? {
                ContextResponse::Outcome(outcome) => Ok(outcome),
                ContextResponse::Error(error) => Err(ContextClientError::Rejected(error)),
                _ => Err(ContextClientError::OutcomeUnknown),
            }
        }

        /// Read historical metadata only. Does not resume or retry the request.
        /// # Panics
        /// Tokio may panic unless the current runtime has I/O and time enabled.
        pub async fn context_request_status(self, request_id: String) -> ContextResult<ContextOutcome> {
            let operation = OwnerOperation::ContextRequestStatus(ContextRequestStatusRequest { request_id });
            match self.context_exchange(operation).await? {
                ContextResponse::Outcome(outcome) => Ok(outcome),
                ContextResponse::Error(error) => Err(ContextClientError::Rejected(error)),
                _ => Err(Error::InvalidMessage.into()),
            }
        }
    }

    #[cfg(test)]
    mod context_tests {
        include!("transport_context_tests.rs");
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
    pub async fn respond_capabilities(self, _capability: OwnerCapability) -> Result<()> { Err(Error::UnsupportedPlatform) }
    pub async fn respond_context(self, _response: ContextResponse) -> Result<()> { Err(Error::UnsupportedPlatform) }
}
#[cfg(not(target_os = "linux"))]
pub struct OwnerClient;
#[cfg(not(target_os = "linux"))]
impl OwnerClient {
    pub async fn connect(_profile: &ProfileIdentity, _runtime: &RuntimeDirectory, _request_id: u64) -> Result<Self> { Err(Error::UnsupportedPlatform) }
    pub async fn owner_status(self) -> Result<OwnerStatus> { Err(Error::UnsupportedPlatform) }
    pub async fn owner_capabilities(self) -> Result<OwnerCapability> { Err(Error::UnsupportedPlatform) }
    pub async fn context_inspect(self) -> ContextResult<ContextInspection> { Err(Error::UnsupportedPlatform.into()) }
    pub async fn context_set(self, _request: ContextSetRequest) -> ContextResult<ContextOutcome> { Err(Error::UnsupportedPlatform.into()) }
    pub async fn context_request_status(self, _request_id: String) -> ContextResult<ContextOutcome> { Err(Error::UnsupportedPlatform.into()) }
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
