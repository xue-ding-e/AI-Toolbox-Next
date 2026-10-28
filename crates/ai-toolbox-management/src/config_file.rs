//! Private, Linux-only replacement of one existing `config.toml` leaf.
//!
//! This is not an arbitrary-path writer, an auth writer, or complete desktop
//! writer admission. It retains owner/resource permits throughout the effect.
//! Rechecks detect observed drift; they do not eliminate races with processes
//! of the same UID that ignore the resource lease. Only Unix mode and ownership
//! are preserved, not arbitrary ACLs or extended attributes.
//! Failure closes our descriptors but leaves any named temporary file in place:
//! a pathname check followed by unlink could delete an uncooperative replacement.
//! Successful rename consumes the temporary name; no cleanup unlinks any path.

use crate::owner::{ResourcePermit, RuntimeResourceIdentity, WriterPermit};

pub(crate) const MAX_CONFIG_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PublishDisposition {
    /// This attempt has not called rename. A staging file may remain; this is
    /// not a promise of zero filesystem side effects and is not a rollback.
    NotPublished,
    /// Rename was attempted. Never infer that the old config still applies.
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigFileErrorReason {
    UnsupportedPlatform,
    AdmissionRejected,
    MissingConfig,
    UnsafeConfig,
    ConfigChanged,
    ConfigTooLarge,
    NotWritable,
    ReadFailed,
    RandomUnavailable,
    TemporaryCreateFailed,
    TemporaryChanged,
    TemporaryWriteFailed,
    MetadataFailed,
    FileSyncFailed,
    PublishFailed,
    DirectorySyncFailed,
    ReadbackMismatch,
}

/// Errors deliberately contain no path, input bytes, or raw OS diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConfigFileError {
    disposition: PublishDisposition,
    reason: ConfigFileErrorReason,
}

impl ConfigFileError {
    pub(crate) fn disposition(self) -> PublishDisposition {
        self.disposition
    }

    pub(crate) fn reason(self) -> ConfigFileErrorReason {
        self.reason
    }

    fn before_publish(reason: ConfigFileErrorReason) -> Self {
        Self {
            disposition: PublishDisposition::NotPublished,
            reason,
        }
    }

    fn outcome_unknown(mut self) -> Self {
        self.disposition = PublishDisposition::OutcomeUnknown;
        self
    }
}

impl std::fmt::Display for ConfigFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CONFIG_FILE_{:?}_{:?}", self.disposition, self.reason)
    }
}

impl std::error::Error for ConfigFileError {}

type Result<T> = std::result::Result<T, ConfigFileError>;

/// Optimistic, fixed-leaf inspection without a resource lease or write method.
/// The borrowed in-memory gate prevents competing work by this owner, not by
/// another profile or an uncooperative process. Keep the original FD and recheck
/// the snapshot; a later mutation still needs its own real resource lease/CAS.
/// Raw configuration must never acquire a Debug or Serialize implementation.
#[cfg(target_os = "linux")]
pub(crate) struct ConfigSnapshot<'writer> {
    writer: &'writer WriterPermit,
    runtime: RuntimeResourceIdentity,
    bytes: Vec<u8>,
    original: linux::OpenedConfig,
}

#[cfg(target_os = "linux")]
impl<'writer> ConfigSnapshot<'writer> {
    pub(crate) fn read(
        writer: &'writer WriterPermit,
        runtime: &RuntimeResourceIdentity,
    ) -> Result<Self> {
        writer.ensure_current().map_err(|_| {
            ConfigFileError::before_publish(ConfigFileErrorReason::AdmissionRejected)
        })?;
        let directory = runtime.inspection_directory().map_err(|_| {
            ConfigFileError::before_publish(ConfigFileErrorReason::AdmissionRejected)
        })?;
        // Reuse the effect's bounded O_RDONLY/O_NOFOLLOW fixed-leaf reader.
        // Neither this path nor inspection_directory acquires/creates a lease.
        let (original, bytes) = linux::read_config(directory)?;
        let snapshot = Self {
            writer,
            runtime: runtime.clone(),
            bytes,
            original,
        };
        snapshot.readback()?;
        Ok(snapshot)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn readback(&self) -> Result<()> {
        let directory = self.directory()?;
        self.original.ensure_current()?;
        let (current, bytes) = linux::read_config(directory)?;
        if current.stamp != self.original.stamp || bytes != self.bytes {
            return Err(ConfigFileError::before_publish(
                ConfigFileErrorReason::ConfigChanged,
            ));
        }
        self.directory()?;
        self.original.ensure_current()?;
        Ok(())
    }

    fn directory(&self) -> Result<&std::fs::File> {
        self.writer.ensure_current().map_err(|_| {
            ConfigFileError::before_publish(ConfigFileErrorReason::AdmissionRejected)
        })?;
        self.runtime.inspection_directory().map_err(|_| {
            ConfigFileError::before_publish(ConfigFileErrorReason::AdmissionRejected)
        })
    }
}

/// The original descriptor prevents inode reuse while the plan is outstanding.
/// No Debug/Serialize implementation may accidentally expose its raw config.
pub(crate) struct ConfigFile<'permit, 'writer> {
    writer: &'writer WriterPermit,
    resources: &'permit ResourcePermit<'writer>,
    runtime: RuntimeResourceIdentity,
    bytes: Vec<u8>,
    #[cfg(target_os = "linux")]
    original: linux::OpenedConfig,
}

impl<'permit, 'writer> ConfigFile<'permit, 'writer> {
    pub(crate) fn read(
        writer: &'writer WriterPermit,
        resources: &'permit ResourcePermit<'writer>,
        runtime: &RuntimeResourceIdentity,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let directory = resources.config_directory(writer, runtime).map_err(|_| {
                ConfigFileError::before_publish(ConfigFileErrorReason::AdmissionRejected)
            })?;
            let (original, bytes) = linux::read_config(directory)?;
            let snapshot = Self {
                writer,
                resources,
                runtime: runtime.clone(),
                bytes,
                original,
            };
            snapshot.readback()?;
            Ok(snapshot)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (writer, resources, runtime);
            Err(ConfigFileError::before_publish(
                ConfigFileErrorReason::UnsupportedPlatform,
            ))
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Read-only check before journaling an intended projection. Replace still
    /// repeats it immediately before publication; this is not a reservation.
    pub(crate) fn preflight(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;

            let directory = self.directory()?;
            let mode = directory
                .metadata()
                .map_err(|_| {
                    ConfigFileError::before_publish(ConfigFileErrorReason::AdmissionRejected)
                })?
                .mode();
            // Do not let root's DAC bypass turn explicitly read-only fixtures
            // or configs into successful writes.
            if mode & 0o300 != 0o300 || self.original.stamp.mode & 0o600 != 0o600 {
                return Err(ConfigFileError::before_publish(
                    ConfigFileErrorReason::NotWritable,
                ));
            }
            self.readback().map(|_| ())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ConfigFileError::before_publish(
                ConfigFileErrorReason::UnsupportedPlatform,
            ))
        }
    }

    /// Freshly opens only the fixed leaf and proves that the original snapshot
    /// still applies. Useful for a no-op: no cached bytes are reported as proof.
    pub(crate) fn readback(&self) -> Result<Vec<u8>> {
        #[cfg(target_os = "linux")]
        {
            let directory = self.directory()?;
            self.original.ensure_current()?;
            let (current, bytes) = linux::read_config(directory)?;
            if current.stamp != self.original.stamp || bytes != self.bytes {
                return Err(ConfigFileError::before_publish(
                    ConfigFileErrorReason::ConfigChanged,
                ));
            }
            self.directory()?;
            self.original.ensure_current()?;
            Ok(bytes)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ConfigFileError::before_publish(
                ConfigFileErrorReason::UnsupportedPlatform,
            ))
        }
    }

    /// Stages, syncs, replaces and reads back the existing config. Consuming the
    /// snapshot prevents accidental reuse after its inode has been replaced.
    /// No failure after the rename attempt is represented as a known rollback.
    pub(crate) fn replace(self, candidate: &[u8]) -> Result<Vec<u8>> {
        #[cfg(target_os = "linux")]
        {
            if candidate.len() > MAX_CONFIG_BYTES {
                return Err(ConfigFileError::before_publish(
                    ConfigFileErrorReason::ConfigTooLarge,
                ));
            }
            self.preflight()?;
            let directory = self.directory()?;
            let mut temporary = linux::Temporary::create(directory)?;
            temporary.stage(candidate, &self.original.stamp)?;
            self.preflight()?;
            temporary.ensure_current()?;
            // From this line onward, even a rename error is conservative:
            // publish state may not be inferred from a later cleanup/read.
            temporary
                .publish()
                .map_err(ConfigFileError::outcome_unknown)?;
            directory.sync_all().map_err(|_| {
                ConfigFileError::before_publish(ConfigFileErrorReason::DirectorySyncFailed)
                    .outcome_unknown()
            })?;
            self.directory().map_err(ConfigFileError::outcome_unknown)?;
            let (published, bytes) =
                linux::read_config(directory).map_err(ConfigFileError::outcome_unknown)?;
            if !published.stamp.same_inode(&temporary.stamp)
                || !published.stamp.same_permissions(&self.original.stamp)
                || bytes != candidate
            {
                return Err(ConfigFileError::before_publish(
                    ConfigFileErrorReason::ReadbackMismatch,
                )
                .outcome_unknown());
            }
            self.directory().map_err(ConfigFileError::outcome_unknown)?;
            Ok(bytes)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = candidate;
            Err(ConfigFileError::before_publish(
                ConfigFileErrorReason::UnsupportedPlatform,
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn directory(&self) -> Result<&std::fs::File> {
        self.resources
            .config_directory(self.writer, &self.runtime)
            .map_err(|_| ConfigFileError::before_publish(ConfigFileErrorReason::AdmissionRejected))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::{CStr, CString};
    use std::fs::{File, Metadata, Permissions};
    use std::io::{self, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::{FileExt, MetadataExt, PermissionsExt};

    use super::*;

    fn fail(reason: ConfigFileErrorReason) -> ConfigFileError {
        ConfigFileError::before_publish(reason)
    }

    fn config_name() -> &'static CStr {
        // A fixed leaf, not caller-controlled path text.
        CStr::from_bytes_with_nul(b"config.toml\0").expect("fixed config leaf")
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub(super) struct Stamp {
        device: u64,
        inode: u64,
        uid: u32,
        gid: u32,
        pub(super) mode: u32,
        links: u64,
        length: u64,
        modified: (i64, i64),
        changed: (i64, i64),
    }

    impl Stamp {
        fn from_metadata(meta: &Metadata) -> Self {
            Self {
                device: meta.dev(),
                inode: meta.ino(),
                uid: meta.uid(),
                gid: meta.gid(),
                mode: meta.mode(),
                links: meta.nlink(),
                length: meta.len(),
                modified: (meta.mtime(), meta.mtime_nsec()),
                changed: (meta.ctime(), meta.ctime_nsec()),
            }
        }

        pub(super) fn same_inode(&self, other: &Self) -> bool {
            self.device == other.device && self.inode == other.inode
        }

        pub(super) fn same_permissions(&self, other: &Self) -> bool {
            self.uid == other.uid
                && self.gid == other.gid
                && self.mode == other.mode
                && self.links == other.links
        }

        fn validate(&self) -> Result<()> {
            let uid = crate::owner::linux::current_uid()
                .map_err(|_| fail(ConfigFileErrorReason::AdmissionRejected))?;
            if self.mode & libc::S_IFMT != libc::S_IFREG
                || self.uid != uid
                || self.links != 1
                || self.mode & 0o7022 != 0
            {
                return Err(fail(ConfigFileErrorReason::UnsafeConfig));
            }
            if self.length > MAX_CONFIG_BYTES as u64 {
                return Err(fail(ConfigFileErrorReason::ConfigTooLarge));
            }
            Ok(())
        }
    }

    fn stat_leaf(directory: &File, name: &CStr) -> Result<Stamp> {
        let mut metadata = MaybeUninit::<libc::stat>::uninit();
        // SAFETY: The borrowed directory FD is live; the internally produced
        // single leaf is NUL terminated and output storage has the right size.
        if unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                metadata.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            let reason = if io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                ConfigFileErrorReason::MissingConfig
            } else {
                ConfigFileErrorReason::ReadFailed
            };
            return Err(fail(reason));
        }
        // SAFETY: Successful fstatat initialized the complete structure.
        let meta = unsafe { metadata.assume_init() };
        Ok(Stamp {
            device: meta.st_dev as u64,
            inode: meta.st_ino as u64,
            uid: meta.st_uid,
            gid: meta.st_gid,
            mode: meta.st_mode,
            links: meta.st_nlink as u64,
            length: meta.st_size as u64,
            modified: (meta.st_mtime as i64, meta.st_mtime_nsec as i64),
            changed: (meta.st_ctime as i64, meta.st_ctime_nsec as i64),
        })
    }

    pub(super) struct OpenedConfig {
        file: File,
        pub(super) stamp: Stamp,
    }

    impl OpenedConfig {
        pub(super) fn ensure_current(&self) -> Result<()> {
            let current = Stamp::from_metadata(
                &self
                    .file
                    .metadata()
                    .map_err(|_| fail(ConfigFileErrorReason::ReadFailed))?,
            );
            current.validate()?;
            if current != self.stamp {
                return Err(fail(ConfigFileErrorReason::ConfigChanged));
            }
            Ok(())
        }
    }

    fn read_bounded(file: &File, stamp: &Stamp) -> Result<Vec<u8>> {
        stamp.validate()?;
        // Allocate and read at most 1 MiB, including for a file that grows while
        // being read. Revalidated size/timestamps detect observed growth/drift.
        let mut bytes = vec![0; stamp.length as usize];
        let mut offset = 0;
        while offset < bytes.len() {
            match file.read_at(&mut bytes[offset..], offset as u64) {
                Ok(0) => return Err(fail(ConfigFileErrorReason::ConfigChanged)),
                Ok(count) => offset += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(fail(ConfigFileErrorReason::ReadFailed)),
            }
        }
        let after = Stamp::from_metadata(
            &file
                .metadata()
                .map_err(|_| fail(ConfigFileErrorReason::ReadFailed))?,
        );
        if after != *stamp {
            return Err(fail(ConfigFileErrorReason::ConfigChanged));
        }
        Ok(bytes)
    }

    pub(super) fn read_config(directory: &File) -> Result<(OpenedConfig, Vec<u8>)> {
        let name = config_name();
        let before = stat_leaf(directory, name)?;
        // Reject an already-observed special file without opening it. O_NONBLOCK
        // and O_NOFOLLOW also keep a racing FIFO/link from blocking/following.
        before.validate()?;
        if before.device
            != directory
                .metadata()
                .map_err(|_| fail(ConfigFileErrorReason::ReadFailed))?
                .dev()
        {
            return Err(fail(ConfigFileErrorReason::UnsafeConfig));
        }
        // SAFETY: The directory FD and fixed C leaf are valid for this call.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(fail(ConfigFileErrorReason::ReadFailed));
        }
        // SAFETY: Successful openat returned one new owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        let held = Stamp::from_metadata(
            &file
                .metadata()
                .map_err(|_| fail(ConfigFileErrorReason::ReadFailed))?,
        );
        held.validate()?;
        if held != before {
            return Err(fail(ConfigFileErrorReason::ConfigChanged));
        }
        let bytes = read_bounded(&file, &held)?;
        if stat_leaf(directory, name)? != held {
            return Err(fail(ConfigFileErrorReason::ConfigChanged));
        }
        Ok((OpenedConfig { file, stamp: held }, bytes))
    }

    pub(super) struct Temporary<'directory> {
        directory: &'directory File,
        name: CString,
        file: File,
        pub(super) stamp: Stamp,
    }

    impl<'directory> Temporary<'directory> {
        pub(super) fn create(directory: &'directory File) -> Result<Self> {
            // Bound collision retries. Unknown pre-existing leaves are never
            // opened for writing, truncated, repaired, or removed.
            for _ in 0..8 {
                let mut random = [0u8; 16];
                getrandom::fill(&mut random)
                    .map_err(|_| fail(ConfigFileErrorReason::RandomUnavailable))?;
                let hex = random
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let name = CString::new(format!(".ai-toolbox-config-{hex}.tmp"))
                    .map_err(|_| fail(ConfigFileErrorReason::TemporaryCreateFailed))?;
                // SAFETY: The directory FD is live and the locally generated
                // name is one C leaf. Exclusive creation cannot open an old file.
                let fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDWR
                            | libc::O_CREAT
                            | libc::O_EXCL
                            | libc::O_CLOEXEC
                            | libc::O_NOFOLLOW
                            | libc::O_NONBLOCK,
                        0o600 as libc::mode_t,
                    )
                };
                if fd < 0 {
                    if io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists {
                        continue;
                    }
                    return Err(fail(ConfigFileErrorReason::TemporaryCreateFailed));
                }
                // SAFETY: Successful exclusive openat returned a new owned FD.
                let file = unsafe { File::from_raw_fd(fd) };
                let stamp = Stamp::from_metadata(
                    &file
                        .metadata()
                        .map_err(|_| fail(ConfigFileErrorReason::MetadataFailed))?,
                );
                stamp.validate()?;
                let temporary = Self {
                    directory,
                    name,
                    file,
                    stamp,
                };
                temporary.ensure_current()?;
                return Ok(temporary);
            }
            Err(fail(ConfigFileErrorReason::TemporaryCreateFailed))
        }

        pub(super) fn stage(&mut self, bytes: &[u8], original: &Stamp) -> Result<()> {
            self.file
                .write_all(bytes)
                .map_err(|_| fail(ConfigFileErrorReason::TemporaryWriteFailed))?;
            if self.stamp.gid != original.gid {
                // SAFETY: This is our exclusively created file. An unchanged
                // UID and original GID preserve access rather than widening it.
                if unsafe { libc::fchown(self.file.as_raw_fd(), !0 as libc::uid_t, original.gid) }
                    != 0
                {
                    return Err(fail(ConfigFileErrorReason::MetadataFailed));
                }
            }
            self.file
                .set_permissions(Permissions::from_mode(original.mode & 0o777))
                .map_err(|_| fail(ConfigFileErrorReason::MetadataFailed))?;
            self.file
                .sync_all()
                .map_err(|_| fail(ConfigFileErrorReason::FileSyncFailed))?;
            let staged = Stamp::from_metadata(
                &self
                    .file
                    .metadata()
                    .map_err(|_| fail(ConfigFileErrorReason::MetadataFailed))?,
            );
            staged.validate()?;
            if !staged.same_inode(&self.stamp)
                || !staged.same_permissions(original)
                || staged.length != bytes.len() as u64
            {
                return Err(fail(ConfigFileErrorReason::TemporaryChanged));
            }
            self.stamp = staged;
            if read_bounded(&self.file, &self.stamp)? != bytes {
                return Err(fail(ConfigFileErrorReason::TemporaryChanged));
            }
            self.ensure_current()
        }

        pub(super) fn ensure_current(&self) -> Result<()> {
            let held = Stamp::from_metadata(
                &self
                    .file
                    .metadata()
                    .map_err(|_| fail(ConfigFileErrorReason::MetadataFailed))?,
            );
            held.validate()?;
            if held != self.stamp || stat_leaf(self.directory, &self.name)? != held {
                return Err(fail(ConfigFileErrorReason::TemporaryChanged));
            }
            Ok(())
        }

        pub(super) fn publish(&mut self) -> Result<()> {
            // SAFETY: Both names are internal single leaves in the same held
            // directory. renameat atomically replaces the config name only.
            if unsafe {
                libc::renameat(
                    self.directory.as_raw_fd(),
                    self.name.as_ptr(),
                    self.directory.as_raw_fd(),
                    config_name().as_ptr(),
                )
            } != 0
            {
                return Err(fail(ConfigFileErrorReason::PublishFailed));
            }
            Ok(())
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::{symlink, MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    use crate::owner::{OwnerPermit, ProfileIdentity, ProfileOwner};

    use super::*;

    const ORIGINAL: &[u8] = b"model = \"synthetic-original\"\n";
    const CANDIDATE: &[u8] = b"model = \"synthetic-candidate\"\n";
    const AUTH_SENTINEL: &[u8] = b"synthetic-auth-sentinel-not-a-credential";

    fn write_private(path: &Path, bytes: &[u8]) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .unwrap();
        file.write_all(bytes).unwrap();
    }

    struct Fixture {
        // Drop owner/runtime descriptors before the owned directory tree.
        owner: OwnerPermit,
        runtime: RuntimeResourceIdentity,
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new(config: Option<&[u8]>) -> Self {
            // The runner supplies a supported local TMPDIR. Never fall back to
            // another root or a real profile if owner admission rejects it.
            let root = tempfile::Builder::new()
                .prefix("ait-config-")
                .permissions(fs::Permissions::from_mode(0o700))
                .tempdir()
                .unwrap();
            assert_eq!(fs::metadata(root.path()).unwrap().mode() & 0o7777, 0o700);
            for leaf in ["profile", "runtime"] {
                let path = root.path().join(leaf);
                fs::create_dir(&path).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                assert_eq!(fs::metadata(path).unwrap().mode() & 0o7777, 0o700);
            }
            let runtime_path = root.path().join("runtime");
            if let Some(bytes) = config {
                write_private(&runtime_path.join("config.toml"), bytes);
            }
            write_private(&runtime_path.join("auth.json"), AUTH_SENTINEL);
            let owner = ProfileOwner::acquire(
                ProfileIdentity::existing(root.path().join("profile")).unwrap(),
            )
            .unwrap();
            let runtime = RuntimeResourceIdentity::existing(runtime_path).unwrap();
            Self {
                owner,
                runtime,
                root,
            }
        }

        fn path(&self, leaf: &str) -> PathBuf {
            self.runtime.canonical_path().join(leaf)
        }

        fn assert_auth_unchanged(&self, inode: u64) {
            assert_eq!(fs::read(self.path("auth.json")).unwrap(), AUTH_SENTINEL);
            assert_eq!(fs::metadata(self.path("auth.json")).unwrap().ino(), inode);
        }
    }

    #[test]
    fn inspection_snapshot_reads_read_only_files_without_creating_resource_lease() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let auth_inode = fs::metadata(fixture.path("auth.json")).unwrap().ino();
        fs::set_permissions(fixture.path("config.toml"), fs::Permissions::from_mode(0o400)).unwrap();
        fs::set_permissions(fixture.runtime.canonical_path(), fs::Permissions::from_mode(0o500)).unwrap();
        let writer = fixture.owner.try_writer().unwrap();
        let snapshot = ConfigSnapshot::read(&writer, &fixture.runtime).unwrap();
        assert_eq!(snapshot.bytes(), ORIGINAL);
        snapshot.readback().unwrap();
        assert!(!fixture.path(crate::owner::RESOURCE_LEASE_FILE).exists());
        assert_eq!(fs::read_dir(fixture.runtime.canonical_path()).unwrap().count(), 2);
        assert_eq!(fs::metadata(fixture.path("config.toml")).unwrap().mode() & 0o7777, 0o400);
        fixture.assert_auth_unchanged(auth_inode);
        drop(snapshot);
        drop(writer);
        // Restore only this synthetic directory for TempDir's owned cleanup.
        fs::set_permissions(fixture.runtime.canonical_path(), fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn inspection_snapshot_reuses_missing_special_file_and_size_rejections() {
        for case in ["missing", "symlink", "hardlink", "fifo", "oversized"] {
            let fixture = Fixture::new(None);
            let path = fixture.path("config.toml");
            let expected = match case {
                "missing" => ConfigFileErrorReason::MissingConfig,
                "symlink" => {
                    symlink(fixture.path("auth.json"), &path).unwrap();
                    ConfigFileErrorReason::UnsafeConfig
                }
                "hardlink" => {
                    fs::hard_link(fixture.path("auth.json"), &path).unwrap();
                    ConfigFileErrorReason::UnsafeConfig
                }
                "fifo" => {
                    use std::os::unix::ffi::OsStrExt;
                    let leaf = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
                    // SAFETY: The owned fixture path is NUL terminated; this
                    // creates only one synthetic FIFO, never a production leaf.
                    assert_eq!(unsafe { libc::mkfifo(leaf.as_ptr(), 0o600) }, 0);
                    ConfigFileErrorReason::UnsafeConfig
                }
                _ => {
                    write_private(&path, &vec![b'x'; MAX_CONFIG_BYTES + 1]);
                    ConfigFileErrorReason::ConfigTooLarge
                }
            };
            let writer = fixture.owner.try_writer().unwrap();
            let error = ConfigSnapshot::read(&writer, &fixture.runtime).err().unwrap();
            assert_eq!(error.reason(), expected);
            assert!(!fixture.path(crate::owner::RESOURCE_LEASE_FILE).exists());
            if case == "missing" {
                assert!(!path.exists());
            }
        }
    }

    #[test]
    fn inspection_snapshot_detects_content_and_same_bytes_inode_replacement() {
        for replace_inode in [false, true] {
            let fixture = Fixture::new(Some(ORIGINAL));
            let writer = fixture.owner.try_writer().unwrap();
            let snapshot = ConfigSnapshot::read(&writer, &fixture.runtime).unwrap();
            if replace_inode {
                fs::rename(fixture.path("config.toml"), fixture.path("original.toml")).unwrap();
                write_private(&fixture.path("config.toml"), ORIGINAL);
            } else {
                fs::write(fixture.path("config.toml"), CANDIDATE).unwrap();
            }
            assert_eq!(snapshot.readback().unwrap_err().reason(), ConfigFileErrorReason::ConfigChanged);
            assert!(!fixture.path(crate::owner::RESOURCE_LEASE_FILE).exists());
        }
    }

    #[test]
    fn real_replace_preserves_mode_owner_and_auth_and_returns_readback() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let before = fs::metadata(fixture.path("config.toml")).unwrap();
        let auth_inode = fs::metadata(fixture.path("auth.json")).unwrap().ino();
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let snapshot = ConfigFile::read(&writer, &resources, &fixture.runtime).unwrap();
        assert_eq!(snapshot.bytes(), ORIGINAL);
        snapshot.preflight().unwrap();
        assert_eq!(snapshot.replace(CANDIDATE).unwrap(), CANDIDATE);
        let after = fs::metadata(fixture.path("config.toml")).unwrap();
        assert_ne!(after.ino(), before.ino());
        assert_eq!(
            (after.mode(), after.uid(), after.gid(), after.nlink()),
            (before.mode(), before.uid(), before.gid(), 1)
        );
        assert_eq!(after.mode() & 0o7777, 0o600);
        assert_eq!(fs::read(fixture.path("config.toml")).unwrap(), CANDIDATE);
        fixture.assert_auth_unchanged(auth_inode);
        assert!(!fs::read_dir(fixture.runtime.canonical_path())
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".ai-toolbox-config-")));
    }

    #[test]
    fn missing_config_is_rejected_without_creating_it() {
        let fixture = Fixture::new(None);
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let error = ConfigFile::read(&writer, &resources, &fixture.runtime)
            .err()
            .expect("missing config must fail");
        assert_eq!(error.reason(), ConfigFileErrorReason::MissingConfig);
        assert_eq!(error.disposition(), PublishDisposition::NotPublished);
        assert!(!fixture.path("config.toml").exists());
    }

    #[test]
    fn symlink_config_is_rejected_without_mutating_link_or_target() {
        let fixture = Fixture::new(None);
        let outside = fixture.root.path().join("outside.toml");
        write_private(&outside, ORIGINAL);
        symlink(&outside, fixture.path("config.toml")).unwrap();
        let before = fs::metadata(&outside).unwrap();
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let error = ConfigFile::read(&writer, &resources, &fixture.runtime)
            .err()
            .expect("symlink must fail");
        assert_eq!(error.reason(), ConfigFileErrorReason::UnsafeConfig);
        assert_eq!(error.disposition(), PublishDisposition::NotPublished);
        assert!(fs::symlink_metadata(fixture.path("config.toml"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::metadata(&outside).unwrap().ino(), before.ino());
        assert_eq!(fs::read(&outside).unwrap(), ORIGINAL);
    }

    #[test]
    fn hardlinked_config_is_rejected_without_mutating_either_name() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let alias = fixture.root.path().join("alias.toml");
        fs::hard_link(fixture.path("config.toml"), &alias).unwrap();
        let before = fs::metadata(&alias).unwrap();
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let error = ConfigFile::read(&writer, &resources, &fixture.runtime)
            .err()
            .expect("hardlink must fail");
        assert_eq!(error.reason(), ConfigFileErrorReason::UnsafeConfig);
        assert_eq!(error.disposition(), PublishDisposition::NotPublished);
        assert_eq!(
            fs::metadata(fixture.path("config.toml")).unwrap().ino(),
            before.ino()
        );
        assert_eq!(fs::metadata(&alias).unwrap().nlink(), 2);
        assert_eq!(fs::read(fixture.path("config.toml")).unwrap(), ORIGINAL);
        assert_eq!(fs::read(&alias).unwrap(), ORIGINAL);
    }

    #[test]
    fn snapshot_inode_drift_rejects_replace_even_with_identical_bytes() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let snapshot = ConfigFile::read(&writer, &resources, &fixture.runtime).unwrap();
        let retired = fixture.path("retired-original.toml");
        fs::rename(fixture.path("config.toml"), &retired).unwrap();
        write_private(&fixture.path("config.toml"), ORIGINAL);
        let replacement_inode = fs::metadata(fixture.path("config.toml")).unwrap().ino();
        let error = snapshot.replace(CANDIDATE).unwrap_err();
        assert_eq!(error.disposition(), PublishDisposition::NotPublished);
        assert_eq!(
            fs::metadata(fixture.path("config.toml")).unwrap().ino(),
            replacement_inode
        );
        assert_eq!(fs::read(fixture.path("config.toml")).unwrap(), ORIGINAL);
        assert_eq!(fs::read(retired).unwrap(), ORIGINAL);
    }

    #[test]
    fn snapshot_content_drift_rejects_replace_on_the_same_inode() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let snapshot = ConfigFile::read(&writer, &resources, &fixture.runtime).unwrap();
        let inode = fs::metadata(fixture.path("config.toml")).unwrap().ino();
        let external = b"external_synthetic_change = true\n";
        fs::write(fixture.path("config.toml"), external).unwrap();
        let error = snapshot.replace(CANDIDATE).unwrap_err();
        assert_eq!(error.disposition(), PublishDisposition::NotPublished);
        assert_eq!(
            fs::metadata(fixture.path("config.toml")).unwrap().ino(),
            inode
        );
        assert_eq!(fs::read(fixture.path("config.toml")).unwrap(), external);
    }

    #[test]
    fn noop_readback_is_fresh_and_does_not_replace_the_inode() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let snapshot = ConfigFile::read(&writer, &resources, &fixture.runtime).unwrap();
        let inode = fs::metadata(fixture.path("config.toml")).unwrap().ino();
        snapshot.preflight().unwrap();
        assert_eq!(snapshot.readback().unwrap(), ORIGINAL);
        assert_eq!(
            fs::metadata(fixture.path("config.toml")).unwrap().ino(),
            inode
        );
        fs::write(fixture.path("config.toml"), b"changed_after_noop = true\n").unwrap();
        assert_eq!(
            snapshot.readback().unwrap_err().disposition(),
            PublishDisposition::NotPublished
        );
    }

    #[test]
    fn successful_replace_preserves_unknown_temp_leaf() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let unknown = fixture.path(".ai-toolbox-config-0123456789abcdef0123456789abcdef.tmp");
        write_private(&unknown, b"unknown-synthetic-temp");
        let inode = fs::metadata(&unknown).unwrap().ino();
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        ConfigFile::read(&writer, &resources, &fixture.runtime)
            .unwrap()
            .replace(CANDIDATE)
            .unwrap();
        assert_eq!(fs::metadata(&unknown).unwrap().ino(), inode);
        assert_eq!(fs::read(unknown).unwrap(), b"unknown-synthetic-temp");
    }

    #[test]
    fn closing_staged_file_does_not_unlink_an_unknown_replacement() {
        let fixture = Fixture::new(Some(ORIGINAL));
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let snapshot = ConfigFile::read(&writer, &resources, &fixture.runtime).unwrap();
        let directory = resources
            .config_directory(&writer, &fixture.runtime)
            .unwrap();
        let mut temporary = linux::Temporary::create(directory).unwrap();
        temporary
            .stage(CANDIDATE, &snapshot.original.stamp)
            .unwrap();
        let named = fs::read_dir(fixture.runtime.canonical_path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".ai-toolbox-config-")
            })
            .expect("exclusive staging created one temp leaf");
        let owned = fixture.path("owned-staged-file");
        fs::rename(&named, &owned).unwrap();
        write_private(&named, b"unknown-replacement");
        let replacement_inode = fs::metadata(&named).unwrap().ino();
        drop(temporary);
        assert_eq!(fs::metadata(&named).unwrap().ino(), replacement_inode);
        assert_eq!(fs::read(named).unwrap(), b"unknown-replacement");
        assert_eq!(fs::read(owned).unwrap(), CANDIDATE);
        assert_eq!(fs::read(fixture.path("config.toml")).unwrap(), ORIGINAL);
    }

    #[test]
    fn resource_permit_cannot_authorize_a_different_writer() {
        let first = Fixture::new(Some(ORIGINAL));
        let second = Fixture::new(Some(ORIGINAL));
        let wrong_writer = first.owner.try_writer().unwrap();
        let writer = second.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![second.runtime.clone()])
            .unwrap();
        let error = ConfigFile::read(&wrong_writer, &resources, &second.runtime)
            .err()
            .expect("writer mismatch must fail");
        assert_eq!(error.reason(), ConfigFileErrorReason::AdmissionRejected);
        assert_eq!(error.disposition(), PublishDisposition::NotPublished);
        assert_eq!(fs::read(second.path("config.toml")).unwrap(), ORIGINAL);
    }

    #[test]
    fn oversized_source_and_candidate_are_rejected_without_replacement() {
        let oversized = vec![b'x'; MAX_CONFIG_BYTES + 1];
        let fixture = Fixture::new(Some(&oversized));
        let writer = fixture.owner.try_writer().unwrap();
        let resources = writer
            .acquire_resources(vec![fixture.runtime.clone()])
            .unwrap();
        let error = ConfigFile::read(&writer, &resources, &fixture.runtime)
            .err()
            .expect("oversized source must fail");
        assert_eq!(error.reason(), ConfigFileErrorReason::ConfigTooLarge);
        fs::write(fixture.path("config.toml"), ORIGINAL).unwrap();
        let inode = fs::metadata(fixture.path("config.toml")).unwrap().ino();
        let snapshot = ConfigFile::read(&writer, &resources, &fixture.runtime).unwrap();
        let error = snapshot.replace(&oversized).unwrap_err();
        assert_eq!(error.reason(), ConfigFileErrorReason::ConfigTooLarge);
        assert_eq!(error.disposition(), PublishDisposition::NotPublished);
        assert_eq!(
            fs::metadata(fixture.path("config.toml")).unwrap().ino(),
            inode
        );
        assert_eq!(fs::read(fixture.path("config.toml")).unwrap(), ORIGINAL);
    }
}
