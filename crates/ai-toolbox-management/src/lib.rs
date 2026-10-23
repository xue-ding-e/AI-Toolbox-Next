//! Candidate local owner boundary. Desktop writers are not connected here.
//!
//! Linux is the only implemented transport. Windows named-pipe ACL and peer
//! verification, and other platforms' native proofs, remain unsupported.

// Private until the complete application writer admission map is connected.
#[cfg(target_os = "linux")]
mod codex_master;
#[cfg(target_os = "linux")]
mod config_file;
#[cfg(target_os = "linux")]
mod codex_service;

pub mod coordination;
pub mod owner;
pub mod transport;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Native owner isolation is not implemented on this platform")]
    UnsupportedPlatform,
    #[error("An explicit absolute directory path is required")]
    RelativePath,
    #[error("The directory filesystem is not verified as a supported local filesystem")]
    UnsupportedFilesystem,
    #[error("The existing profile directory is unavailable or unsafe")]
    InvalidProfile,
    #[error("The runtime directory or its parent is not safe for this user")]
    UnsafeDirectory,
    #[error("A directory or lease inode changed after validation")]
    IdentityChanged,
    #[error("The lease file is not a private, single-link regular file")]
    UnsafeLockFile,
    #[error("Another owner holds this profile")]
    OwnerBusy,
    #[error("Another operation holds this runtime resource")]
    ResourceBusy,
    #[error("A profile writer is already active; pass its permit to nested work")]
    WriterBusy,
    #[error("The endpoint already exists; ownership cannot be safely inferred")]
    EndpointConflict,
    #[error("The owner endpoint is unavailable")]
    OwnerUnavailable,
    #[error("The peer is not the current operating-system user")]
    PeerRejected,
    #[error("The owner protocol version is incompatible")]
    ProtocolMismatch,
    #[error("The owner message is invalid")]
    InvalidMessage,
    #[error("The owner epoch or profile does not match this connection")]
    StaleOwner,
    #[error("The owner frame exceeds the protocol size limit")]
    FrameTooLarge,
    #[error("The local owner operation timed out")]
    TimedOut,
    #[error("No current Tokio runtime is available")]
    RuntimeUnavailable,
    #[error("Operating-system randomness is unavailable")]
    RandomUnavailable,
    #[error("Local owner I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
