//! Capabilities retain the process-lifetime lease. This is not a writer audit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::{Error, Result};

// These persistent coordination files must be excluded from backup, restore,
// import and sync enumerations. Neither file is ever unlinked by this crate.
pub const PROFILE_LEASE_FILE: &str = ".ai-toolbox-owner.lock";
pub const RESOURCE_LEASE_FILE: &str = ".ai-toolbox-runtime.lock";

/// Physical directory identity, not a caller-supplied profile label or PID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileKey {
    pub device: u64,
    pub inode: u64,
}

/// Random lifetime identifier. It is correlation data, not authentication.
pub type OwnerEpoch = [u8; 16];

#[derive(Clone, Debug)]
pub struct ProfileIdentity {
    path: PathBuf,
    key: ProfileKey,
    #[cfg(target_os = "linux")]
    directory: Arc<linux::CheckedDirectory>,
}

impl ProfileIdentity {
    /// Read-only resolution of an explicit absolute path on a supported local
    /// filesystem. Missing profiles must never be created by clients.
    /// Symlink aliases resolve to the same held directory inode.
    pub fn existing(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let directory = Arc::new(linux::CheckedDirectory::profile(path.as_ref())?);
            Ok(Self {
                path: directory.path.clone(),
                key: directory.key,
                directory,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            Err(Error::UnsupportedPlatform)
        }
    }

    /// Database bootstrap must use this path, not resolve the original alias again.
    pub fn canonical_path(&self) -> &Path {
        &self.path
    }

    pub fn key(&self) -> ProfileKey {
        self.key
    }

    pub fn ensure_current(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.directory.ensure_current()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Error::UnsupportedPlatform)
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeDirectory {
    path: PathBuf,
    #[cfg(target_os = "linux")]
    directory: Arc<linux::CheckedDirectory>,
}

impl RuntimeDirectory {
    /// Client path: validates only; never creates directories or lock files.
    pub fn existing(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let directory = Arc::new(linux::CheckedDirectory::private(path.as_ref())?);
            Ok(Self { path: directory.path.clone(), directory })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            Err(Error::UnsupportedPlatform)
        }
    }

    /// GUI-only bootstrap: creates one private leaf under a verified parent.
    /// Existing permissions are never repaired or widened.
    pub fn create_for_owner(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let canonical = linux::create_private_leaf(path.as_ref())?;
            Self::existing(canonical)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            Err(Error::UnsupportedPlatform)
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn ensure_current(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.directory.ensure_current()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Error::UnsupportedPlatform)
        }
    }
}

pub struct ProfileOwner;

impl ProfileOwner {
    /// Must succeed before opening, migrating, or seeding this profile's DB.
    pub fn acquire(profile: ProfileIdentity) -> Result<OwnerPermit> {
        #[cfg(target_os = "linux")]
        {
            let lease = linux::Lease::acquire(&profile.directory, PROFILE_LEASE_FILE)?;
            let mut epoch = [0; 16];
            getrandom::fill(&mut epoch).map_err(|_| Error::RandomUnavailable)?;
            Ok(OwnerPermit {
                inner: Arc::new(OwnerInner {
                    profile,
                    epoch,
                    gate: Arc::new(Mutex::new(())),
                    lease,
                }),
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = profile;
            Err(Error::UnsupportedPlatform)
        }
    }
}

struct OwnerInner {
    profile: ProfileIdentity,
    epoch: OwnerEpoch,
    gate: Arc<Mutex<()>>,
    #[cfg(target_os = "linux")]
    lease: linux::Lease,
}

/// Clone capabilities, never duplicate the underlying lock's File descriptor.
/// Keep a capability with DB state and with every in-flight owner operation.
#[derive(Clone)]
pub struct OwnerPermit {
    inner: Arc<OwnerInner>,
}

impl OwnerPermit {
    // Preserve the allocation identity, not just an epoch/profile label. The
    // storage crate retains this lease without depending on management types.
    #[cfg(target_os = "linux")]
    pub(crate) fn lifetime_owner(&self) -> Arc<dyn Send + Sync> {
        self.inner.clone()
    }

    pub fn profile(&self) -> &ProfileIdentity {
        &self.inner.profile
    }

    pub fn epoch(&self) -> OwnerEpoch {
        self.inner.epoch
    }

    pub fn ensure_current(&self) -> Result<()> {
        self.profile().ensure_current()?;
        #[cfg(target_os = "linux")]
        {
            self.inner.lease.ensure_current()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Error::UnsupportedPlatform)
        }
    }

    /// Non-reentrant by design. Inner business functions accept &WriterPermit
    /// instead of reacquiring a gate; recursive acquisition fails rather than hangs.
    pub fn try_writer(&self) -> Result<WriterPermit> {
        self.ensure_current()?;
        let gate = self.inner.gate.clone().try_lock_owned().map_err(|_| Error::WriterBusy)?;
        // Recheck after entering the gate; no side effect precedes this proof.
        self.ensure_current()?;
        Ok(WriterPermit { _gate: gate, owner: self.clone() })
    }
}

pub struct WriterPermit {
    // Rust drops fields in declaration order: release the gate before the owner.
    _gate: OwnedMutexGuard<()>,
    owner: OwnerPermit,
}

impl WriterPermit {
    pub fn profile(&self) -> &ProfileIdentity {
        self.owner.profile()
    }

    /// A permit from a different owner/profile cannot authorize this service.
    pub fn require_owner(&self, owner: &OwnerPermit) -> Result<()> {
        if !Arc::ptr_eq(&self.owner.inner, &owner.inner) {
            return Err(Error::StaleOwner);
        }
        self.owner.ensure_current()
    }

    pub fn ensure_current(&self) -> Result<()> {
        self.owner.ensure_current()
    }

    /// Take one cross-profile lease per physical runtime directory. All roots
    /// must already exist. No lock directory fallback is permitted for a
    /// read-only root, since that would silently split the coordination domain.
    pub fn acquire_resources(&self, mut resources: Vec<RuntimeResourceIdentity>) -> Result<ResourcePermit<'_>> {
        self.ensure_current()?;
        resources.sort_by_key(|resource| (resource.key().device, resource.key().inode));
        resources.dedup_by_key(|resource| (resource.key().device, resource.key().inode));
        #[cfg(target_os = "linux")]
        {
            let mut leases = Vec::with_capacity(resources.len());
            for resource in &resources {
                let lease = linux::Lease::acquire(&resource.0.directory, RESOURCE_LEASE_FILE)
                    .map_err(|error| match error { Error::OwnerBusy => Error::ResourceBusy, other => other })?;
                leases.push(lease);
            }
            // A failure above drops only this attempt's acquired descriptors.
            Ok(ResourcePermit { writer: self, resources, leases })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = resources;
            Err(Error::UnsupportedPlatform)
        }
    }
}

/// The resource identity deliberately excludes the selected profile and IPC
/// directory. Symlink aliases and profiles targeting one root contend together.
#[derive(Clone, Debug)]
pub struct RuntimeResourceIdentity(ProfileIdentity);

impl RuntimeResourceIdentity {
    pub fn existing(path: impl AsRef<Path>) -> Result<Self> {
        ProfileIdentity::existing(path).map(Self)
    }

    pub fn canonical_path(&self) -> &Path { self.0.canonical_path() }
    pub fn key(&self) -> ProfileKey { self.0.key() }
    pub fn ensure_current(&self) -> Result<()> { self.0.ensure_current() }
}

/// Borrows the writer so the profile gate and owner cannot end first. Nested
/// projection helpers receive this permit instead of locking the same root again.
pub struct ResourcePermit<'a> {
    writer: &'a WriterPermit,
    resources: Vec<RuntimeResourceIdentity>,
    #[cfg(target_os = "linux")]
    leases: Vec<linux::Lease>,
}

impl ResourcePermit<'_> {
    pub fn require_resource(&self, resource: &RuntimeResourceIdentity) -> Result<()> {
        self.writer.ensure_current()?;
        resource.ensure_current()?;
        if !self.resources.iter().any(|held| held.key() == resource.key()) {
            return Err(Error::IdentityChanged);
        }
        #[cfg(target_os = "linux")]
        for lease in &self.leases { lease.ensure_current()?; }
        Ok(())
    }

    /// Private fixed-config effect capability. The supplied writer must be the
    /// exact writer borrowed by this resource permit, not another live owner.
    /// The borrow retains both permits; callers must revalidate before effects.
    #[cfg(target_os = "linux")]
    pub(crate) fn config_directory<'a>(
        &'a self,
        writer: &WriterPermit,
        resource: &'a RuntimeResourceIdentity,
    ) -> Result<&'a std::fs::File> {
        if !std::ptr::eq(self.writer, writer) {
            return Err(Error::StaleOwner);
        }
        self.require_resource(resource)?;
        Ok(resource.0.directory.config_directory())
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod linux {
    use std::ffi::CString;
    use std::fs::{self, File, Metadata, OpenOptions};
    use std::io;
    use std::mem::MaybeUninit;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    use super::*;

    pub(crate) fn current_uid() -> Result<u32> {
        // SAFETY: These process identity queries have no arguments or pointers.
        let (uid, effective_uid) = unsafe { (libc::getuid(), libc::geteuid()) };
        if uid != effective_uid {
            return Err(Error::PeerRejected);
        }
        Ok(uid)
    }

    pub(crate) fn key(metadata: &Metadata) -> ProfileKey {
        ProfileKey { device: metadata.dev(), inode: metadata.ino() }
    }

    fn require_absolute(path: &Path) -> Result<()> {
        if path.is_absolute() { Ok(()) } else { Err(Error::RelativePath) }
    }

    // Linux UAPI include/uapi/linux/magic.h, also installed as
    // /usr/include/linux/magic.h. EXT2/3/4 share 0xef53: this is an ext-family
    // allowlist, not proof of ext4 specifically. Overlay/FUSE and unknown types
    // do not establish local backing storage and intentionally remain denied.
    fn supported_filesystem_magic(magic: u64) -> bool {
        matches!(magic, 0xef53 | 0x5846_5342 | 0x9123_683e | 0x0102_1994)
    }

    fn require_supported_filesystem(directory: &File) -> Result<()> {
        let mut metadata = MaybeUninit::<libc::statfs>::uninit();
        // SAFETY: The caller retains an O_DIRECTORY descriptor and supplies
        // valid writable storage for the complete statfs structure.
        if unsafe { libc::fstatfs(directory.as_raw_fd(), metadata.as_mut_ptr()) } != 0 {
            return Err(Error::UnsupportedFilesystem);
        }
        // SAFETY: A successful fstatfs initializes the complete output.
        let metadata = unsafe { metadata.assume_init() };
        // c_ulong preserves Linux's native word width, including magic values
        // with bit 31 set on 32-bit targets, without dropping unknown high bits.
        let magic = metadata.f_type as libc::c_ulong as u64;
        if supported_filesystem_magic(magic) { Ok(()) } else { Err(Error::UnsupportedFilesystem) }
    }

    fn open_directory(path: &Path) -> Result<File> {
        let file = OpenOptions::new().read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        // Inspect the held FD, never a separate string-based statfs result.
        require_supported_filesystem(&file)?;
        Ok(file)
    }

    /// Root-owned sticky ancestors such as /tmp are safe for an owned leaf.
    /// Non-sticky group/world-writable ancestors can replace that leaf and fail.
    fn check_ancestors(path: &Path, uid: u32) -> Result<()> {
        for ancestor in path.ancestors() {
            let meta = fs::symlink_metadata(ancestor)?;
            if !meta.is_dir() || (meta.uid() != uid && meta.uid() != 0) {
                return Err(Error::UnsafeDirectory);
            }
            if meta.mode() & 0o022 != 0 && !(meta.uid() == 0 && meta.mode() & 0o1000 != 0) {
                return Err(Error::UnsafeDirectory);
            }
        }
        Ok(())
    }

    #[derive(Debug)]
    pub(crate) struct CheckedDirectory {
        pub(crate) path: PathBuf,
        pub(crate) key: ProfileKey,
        file: File,
        uid: u32,
        private: bool,
    }

    impl CheckedDirectory {
        // This descriptor is not a path-based filesystem API. Only the private
        // permit-gated fixed-config effect borrows it from the owner boundary.
        pub(super) fn config_directory(&self) -> &File {
            &self.file
        }

        pub(crate) fn profile(path: &Path) -> Result<Self> {
            require_absolute(path)?;
            let canonical = fs::canonicalize(path).map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound { Error::InvalidProfile } else { Error::Io(error) }
            })?;
            Self::open(canonical, false)
        }

        pub(crate) fn private(path: &Path) -> Result<Self> {
            require_absolute(path)?;
            if fs::symlink_metadata(path)?.file_type().is_symlink() {
                return Err(Error::UnsafeDirectory);
            }
            let directory = Self::open(fs::canonicalize(path)?, true)?;
            if key(&fs::symlink_metadata(path)?) != directory.key {
                return Err(Error::IdentityChanged);
            }
            Ok(directory)
        }

        fn open(path: PathBuf, private: bool) -> Result<Self> {
            let uid = current_uid()?;
            check_ancestors(&path, uid)?;
            let file = open_directory(&path)?;
            let meta = file.metadata()?;
            let directory = Self { path, key: key(&meta), file, uid, private };
            directory.ensure_current()?;
            Ok(directory)
        }

        pub(crate) fn ensure_current(&self) -> Result<()> {
            if current_uid()? != self.uid {
                return Err(Error::PeerRejected);
            }
            require_supported_filesystem(&self.file)?;
            check_ancestors(&self.path, self.uid)?;
            for meta in [self.file.metadata()?, fs::symlink_metadata(&self.path)?] {
                if !meta.is_dir() || key(&meta) != self.key {
                    return Err(Error::IdentityChanged);
                }
                if meta.uid() != self.uid || meta.mode() & 0o022 != 0
                    || (self.private && meta.mode() & 0o7777 != 0o700)
                {
                    return Err(Error::UnsafeDirectory);
                }
            }
            Ok(())
        }
    }

    pub(crate) fn create_private_leaf(path: &Path) -> Result<PathBuf> {
        require_absolute(path)?;
        if path.file_name().is_none() {
            return Err(Error::UnsafeDirectory);
        }
        let parent = fs::canonicalize(path.parent().ok_or(Error::UnsafeDirectory)?)?;
        check_ancestors(&parent, current_uid()?)?;
        // Reject unsupported parent filesystems before any leaf is created.
        let parent_file = open_directory(&parent)?;
        let parent_key = key(&parent_file.metadata()?);
        if key(&fs::symlink_metadata(&parent)?) != parent_key {
            return Err(Error::IdentityChanged);
        }
        let canonical = parent.join(path.file_name().ok_or(Error::UnsafeDirectory)?);
        let leaf = CString::new(path.file_name().ok_or(Error::UnsafeDirectory)?.as_bytes())
            .map_err(|_| Error::UnsafeDirectory)?;
        // No recursive creation and no chmod of an existing directory.
        // Anchor creation to the verified local parent FD, not a mutable alias.
        // SAFETY: The held directory FD and NUL-terminated single leaf are valid.
        if unsafe { libc::mkdirat(parent_file.as_raw_fd(), leaf.as_ptr(), 0o700) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error.into());
            }
        }
        if key(&fs::symlink_metadata(&parent)?) != parent_key {
            return Err(Error::IdentityChanged);
        }
        Ok(canonical)
    }

    pub(crate) struct Lease {
        file: File,
        directory: Arc<CheckedDirectory>,
        path: PathBuf,
        key: ProfileKey,
    }

    impl Lease {
        pub(crate) fn acquire(directory: &Arc<CheckedDirectory>, name: &str) -> Result<Self> {
            directory.ensure_current()?;
            if directory.file.metadata()?.mode() & 0o300 != 0o300 {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied).into());
            }
            let name_c = CString::new(name).map_err(|_| Error::UnsafeLockFile)?;
            let path = directory.path.join(name);
            let previous_key = match fs::symlink_metadata(&path) {
                Ok(meta) => {
                    validate_lock(&meta, directory.uid)?;
                    Some(key(&meta))
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            // Existing special files are rejected before even opening them.
            // An absent leaf uses exclusive creation, so a racing replacement
            // cannot be opened as a different kind of device or lock inode.
            let create_flags = if previous_key.is_none() { libc::O_CREAT | libc::O_EXCL } else { 0 };
            // SAFETY: The directory FD is alive and the fixed leaf name is NUL
            // terminated. O_NOFOLLOW/O_NONBLOCK avoid following links/blocking
            // on special files; fstat below rejects all non-regular objects.
            let fd = unsafe {
                libc::openat(directory.file.as_raw_fd(), name_c.as_ptr(),
                    libc::O_RDWR | create_flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                    0o600 as libc::mode_t)
            };
            if fd < 0 {
                return Err(io::Error::last_os_error().into());
            }
            // SAFETY: openat returned a new descriptor owned only by this File.
            let file = unsafe { File::from_raw_fd(fd) };
            let meta = file.metadata()?;
            if previous_key.is_some_and(|previous| previous != key(&meta)) {
                return Err(Error::IdentityChanged);
            }
            let lease = Self { file, directory: directory.clone(), path, key: key(&meta) };
            lease.ensure_current()?;
            fs2::FileExt::try_lock_exclusive(&lease.file).map_err(|error| {
                if error.kind() == io::ErrorKind::WouldBlock { Error::OwnerBusy } else { Error::Io(error) }
            })?;
            lease.ensure_current()?;
            Ok(lease)
        }

        pub(crate) fn ensure_current(&self) -> Result<()> {
            self.directory.ensure_current()?;
            for meta in [self.file.metadata()?, fs::symlink_metadata(&self.path)?] {
                validate_lock(&meta, self.directory.uid)?;
                if key(&meta) != self.key {
                    return Err(Error::IdentityChanged);
                }
            }
            Ok(())
        }
        // Close releases the advisory lease. NEVER unlink or replace this inode.
    }

    fn validate_lock(meta: &Metadata, uid: u32) -> Result<()> {
        if !meta.is_file() || meta.uid() != uid || meta.nlink() != 1 || meta.mode() & 0o7777 != 0o600 {
            Err(Error::UnsafeLockFile)
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn local_filesystem_allowlist_is_narrow_and_not_overlay_backing_inference() {
            for magic in [0xef53, 0x5846_5342, 0x9123_683e, 0x0102_1994] {
                assert!(supported_filesystem_magic(magic));
            }
            // NFS, SMB, CIFS, SMB2, Overlay, FUSE, unknown and truncated-like
            // values must not accidentally acquire local-filesystem authority.
            for magic in [0x6969, 0x517b, 0xff53_4d42, 0xfe53_4d42, 0x794c_7630, 0x6573_5546, 0, u64::MAX, 0x1_0000_ef53] {
                assert!(!supported_filesystem_magic(magic));
            }
        }

        #[test]
        fn absolute_path_admission_is_pure_and_rejects_relative_forms() {
            assert!(require_absolute(Path::new("/explicit/profile")).is_ok());
            for path in ["", "profile", "./profile", "../profile"] {
                assert!(matches!(require_absolute(Path::new(path)), Err(Error::RelativePath)));
            }
        }
    }
}
