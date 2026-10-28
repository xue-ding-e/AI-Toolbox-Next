//! Process-lifetime GUI admission. Not installed or exposed until startup and
//! every affected writer have been connected in the same candidate.
//!
//! A weak registry supports existing helpers without an AppHandle. It is not
//! another owner: accepted operations retain the actual session themselves.
use std::sync::{Arc, OnceLock, Weak};

#[cfg(target_os = "linux")]
use ai_toolbox_management::owner::{OwnerPermit, ProfileLease, WriterPermit};

const UNAVAILABLE: &str = "The GUI profile owner is unavailable";
const MANAGED_UNSUPPORTED: &str = "This operation is not supported while managed Codex writes are enabled";

static SESSION: OnceLock<Weak<GuiProfileSession>> = OnceLock::new();

/// Immutable for a GUI process lifetime; settings cannot promote a live owner.
pub(crate) struct GuiProfileSession {
    #[cfg(target_os = "linux")]
    mode: LinuxMode,
    #[cfg(target_os = "linux")]
    codex_runtime: Option<ai_toolbox_management::owner::RuntimeResourceIdentity>,
    #[cfg(target_os = "linux")]
    ordinary_roots: std::sync::Mutex<Vec<Arc<ai_toolbox_management::owner::OrdinaryRuntimeLeaseGroup>>>,
}

#[cfg(target_os = "linux")]
enum LinuxMode {
    Ordinary(ProfileLease),
    Managed(OwnerPermit),
}

impl GuiProfileSession {
    #[cfg(target_os = "linux")]
    pub(crate) fn retain_database(&self, db: &crate::SqliteDbState) -> Result<(), String> {
        self.ensure_current()?;
        let lease = match &self.mode {
            LinuxMode::Ordinary(lease) => lease,
            LinuxMode::Managed(owner) => owner.profile_lease(),
        };
        lease.retain_database(db).map_err(|_| UNAVAILABLE.to_owned())
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn retain_database(&self, _db: &crate::SqliteDbState) -> Result<(), String> {
        self.ensure_current()
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn ordinary_platform() -> Arc<Self> {
        Arc::new(Self {})
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn ordinary(lease: ProfileLease) -> Arc<Self> {
        Arc::new(Self { mode: LinuxMode::Ordinary(lease), codex_runtime: None, ordinary_roots: std::sync::Mutex::new(Vec::new()) })
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn managed(owner: OwnerPermit) -> Arc<Self> {
        Arc::new(Self { mode: LinuxMode::Managed(owner), codex_runtime: None, ordinary_roots: std::sync::Mutex::new(Vec::new()) })
    }

    /// Production managed startup freezes the same runtime identity used by its
    /// typed service. A session without that identity cannot admit other tools.
    #[cfg(target_os = "linux")]
    pub(crate) fn managed_with_runtime(
        owner: OwnerPermit,
        runtime: ai_toolbox_management::owner::RuntimeResourceIdentity,
    ) -> Result<Arc<Self>, String> {
        owner.ensure_current().map_err(|_| UNAVAILABLE.to_owned())?;
        runtime.ensure_current().map_err(|_| UNAVAILABLE.to_owned())?;
        Ok(Arc::new(Self { mode: LinuxMode::Managed(owner), codex_runtime: Some(runtime), ordinary_roots: std::sync::Mutex::new(Vec::new()) }))
    }

    /// Install only once, before DB/cache/background mutation. The caller must
    /// keep the returned session in application state; this registry is weak.
    pub(crate) fn install(self: &Arc<Self>) -> Result<(), String> {
        self.ensure_current()?;
        SESSION.set(Arc::downgrade(self)).map_err(|_| UNAVAILABLE.to_owned())
    }

    pub(crate) fn ensure_current(&self) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            match &self.mode {
                LinuxMode::Ordinary(lease) => lease.ensure_current(),
                LinuxMode::Managed(owner) => owner.ensure_current(),
            }.map_err(|_| UNAVAILABLE.to_owned())
        }
        #[cfg(not(target_os = "linux"))]
        { Ok(()) }
    }

    #[cfg(target_os = "linux")]
    fn ordinary_runtime(
        self: &Arc<Self>, path: &std::path::Path,
    ) -> Result<Arc<ai_toolbox_management::owner::OrdinaryRuntimeLeaseGroup>, String> {
        self.ensure_current()?;
        let LinuxMode::Ordinary(profile) = &self.mode else {
            return Err(MANAGED_UNSUPPORTED.to_owned());
        };
        let mut roots = self.ordinary_roots.lock().map_err(|_| UNAVAILABLE.to_owned())?;
        // Keep accepted physical roots until this session and its actual workers
        // finish. Nested legacy helpers reuse one allocation, never re-flock it.
        for held in roots.iter() {
            held.ensure_current().map_err(|_| UNAVAILABLE.to_owned())?;
            if held.require_runtime(path).is_ok() { return Ok(held.clone()); }
        }
        let acquired = match std::fs::symlink_metadata(path) {
            Ok(_) => ai_toolbox_management::owner::OrdinaryRuntimeLeaseGroup::acquire(
                profile, vec![path.to_path_buf()],
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound =>
                ai_toolbox_management::owner::OrdinaryRuntimeLeaseGroup::acquire_for_writer_with_bootstrap(profile, path),
            Err(_) => return Err(UNAVAILABLE.to_owned()),
        };
        let held = Arc::new(acquired.map_err(|_| "The Codex runtime cannot be coordinated; no business write was started".to_owned())?);
        roots.push(held.clone());
        Ok(held)
    }

    pub(crate) fn is_managed(&self) -> bool {
        #[cfg(target_os = "linux")]
        { matches!(&self.mode, LinuxMode::Managed(_)) }
        #[cfg(not(target_os = "linux"))]
        { false }
    }
}

/// An expired, previously installed session is an error, never an ordinary
/// mode fallback. Only a cfg(test) unit helper may run without bootstrap;
/// every production platform must install an explicit session first.
fn current_session() -> Result<Option<Arc<GuiProfileSession>>, String> {
    #[cfg(test)]
    if let Some(session) = TEST_SESSION.with(|slot| slot.borrow().clone()) {
        return session.upgrade().map(Some).ok_or_else(|| UNAVAILABLE.to_owned());
    }
    match SESSION.get() {
        #[cfg(test)]
        None => Ok(None),
        #[cfg(not(test))]
        None => Err(UNAVAILABLE.to_owned()),
        Some(weak) => weak.upgrade().map(Some).ok_or_else(|| UNAVAILABLE.to_owned()),
    }
}

/// Observe the installed owner without granting a write capability or creating
/// runtime state. Metadata-only callers retain this session through their work.
pub(crate) fn current_profile_session() -> Result<Option<Arc<GuiProfileSession>>, String> {
    let session = current_session()?;
    if let Some(session) = &session { session.ensure_current()?; }
    Ok(session)
}

/// Reject an unsupported managed operation before its first side effect.
/// Keep this guard inside any blocking worker as well as its outer caller.
/// This is not a resource lease or permission to perform a supported write.
pub(crate) fn reject_managed_operation() -> Result<Option<Arc<GuiProfileSession>>, String> {
    let session = current_session()?;
    if let Some(session) = &session {
        session.ensure_current()?;
        if session.is_managed() {
            return Err(MANAGED_UNSUPPORTED.to_owned());
        }
    }
    Ok(session)
}

/// Retains the ordinary process profile owner. This is not a managed writer
/// capability and does not certify that arbitrary external writers are excluded.
/// Keep it alive for the application lifetime; installation cannot be reset.
pub struct OrdinaryProfileOwnership {
    session: Arc<GuiProfileSession>,
}

impl OrdinaryProfileOwnership {
    /// Retain the same physical profile lease until the final shared DB clone
    /// closes its connection. This does not open or migrate a database.
    pub fn retain_database(&self, db: &crate::SqliteDbState) -> Result<(), String> {
        self.session.retain_database(db)
    }
}

/// Initialize ordinary library/application ownership exactly once, before any
/// DB, cache, background task or business writer is initialized. The profile
/// must already exist. Linux uses the same fixed-inode lease as GUI startup;
/// other platforms retain their existing ordinary behavior, not managed support.
/// This installs no IPC endpoint or ContextSet capability. Dropping the handle
/// does not reset installation: subsequent writer calls fail closed.
pub fn initialize_ordinary_profile(
    profile: &std::path::Path,
) -> Result<OrdinaryProfileOwnership, String> {
    ordinary_startup(profile).map(|session| OrdinaryProfileOwnership { session })
}

/// Build ordinary GUI ownership before opening its DB. This does not publish
/// an endpoint, require Codex configuration, or certify managed readiness.
pub(crate) fn ordinary_startup(profile: &std::path::Path) -> Result<Arc<GuiProfileSession>, String> {
    #[cfg(target_os = "linux")]
    let session = {
        let startup = ai_toolbox_management::owner::ProfileLeaseStartup::acquire(profile)
            .map_err(|_| UNAVAILABLE.to_owned())?;
        GuiProfileSession::ordinary(startup.into_ordinary())
    };
    #[cfg(not(target_os = "linux"))]
    let session = {
        let _ = profile;
        GuiProfileSession::ordinary_platform()
    };
    session.install()?;
    Ok(session)
}

/// Own this for the entire async operation and move it into blocking effects.
/// Nested functions borrow it instead of reacquiring the non-reentrant gate.
pub(crate) struct CodexWriteAdmission {
    session: Option<Arc<GuiProfileSession>>,
    #[cfg(target_os = "linux")]
    writer: Option<WriterPermit>,
}

impl CodexWriteAdmission {
    pub(crate) fn ensure_current(&self) -> Result<(), String> {
        if let Some(session) = &self.session { session.ensure_current()?; }
        #[cfg(target_os = "linux")]
        if let Some(writer) = &self.writer {
            writer.ensure_current().map_err(|_| UNAVAILABLE.to_owned())?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn managed_writer(&self) -> Option<&WriterPermit> { self.writer.as_ref() }
}

/// The caller must already hold DATA_DIR_CHANGE_LOCK when its operation uses
/// that lock. Runtime leases are acquired next, before any DB/file effects.
/// This function alone does not authorize projection.
pub(crate) fn begin_codex_write() -> Result<CodexWriteAdmission, String> {
    let session = current_session()?;
    if let Some(session) = &session { session.ensure_current()?; }
    #[cfg(target_os = "linux")]
    let writer = match session.as_deref().map(|session| &session.mode) {
        Some(LinuxMode::Managed(owner)) => Some(owner.try_writer().map_err(|error| match error {
            ai_toolbox_management::Error::WriterBusy => "Another managed Codex operation is active".to_owned(),
            _ => UNAVAILABLE.to_owned(),
        })?),
        _ => None,
    };
    Ok(CodexWriteAdmission {
        session,
        #[cfg(target_os = "linux")]
        writer,
    })
}

// A test override is scoped to the calling thread, never a production registry
// reset. Async tests using it must remain on the current-thread executor; the
// accepted Arc, not this override, must be moved into blocking effects.
#[cfg(test)]
thread_local! {
    static TEST_SESSION: std::cell::RefCell<Option<Weak<GuiProfileSession>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct TestSessionScope {
    previous: Option<Weak<GuiProfileSession>>,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(test)]
pub(crate) fn test_session(session: &Arc<GuiProfileSession>) -> TestSessionScope {
    let previous = TEST_SESSION.with(|slot| slot.replace(Some(Arc::downgrade(session))));
    TestSessionScope { previous, _thread_bound: std::marker::PhantomData }
}

#[cfg(test)]
impl Drop for TestSessionScope {
    fn drop(&mut self) {
        TEST_SESSION.with(|slot| { slot.replace(self.previous.take()); });
    }
}

/// A target-sensitive admission is retained through the actual filesystem
/// worker. Revalidation never grants a different target or changes permissions.
pub(crate) struct CrossTargetAdmission {
    session: Option<Arc<GuiProfileSession>>,
    targets: Vec<std::path::PathBuf>,
}

impl CrossTargetAdmission {
    pub(crate) fn ensure_current(&self) -> Result<(), String> {
        if let Some(session) = &self.session {
            session.ensure_current()?;
            #[cfg(target_os = "linux")]
            if let LinuxMode::Managed(owner) = &session.mode {
                let runtime = session.codex_runtime.as_ref().ok_or_else(|| UNAVAILABLE.to_owned())?;
                runtime.ensure_current().map_err(|_| UNAVAILABLE.to_owned())?;
                for target in &self.targets {
                    disjoint_target(target, owner.profile().canonical_path(), runtime.canonical_path())?;
                }
                runtime.ensure_current().map_err(|_| UNAVAILABLE.to_owned())?;
            }
        }
        Ok(())
    }
}

pub(crate) fn admit_non_codex_targets(
    tool_key: Option<&str>,
    targets: &[&std::path::Path],
) -> Result<CrossTargetAdmission, String> {
    let session = current_session()?;
    if session.as_ref().is_some_and(|session| session.is_managed())
        && (tool_key == Some("codex") || targets.is_empty())
    {
        return Err(MANAGED_UNSUPPORTED.to_owned());
    }
    let admission = CrossTargetAdmission {
        session,
        targets: targets.iter().map(|path| path.to_path_buf()).collect(),
    };
    admission.ensure_current()?;
    Ok(admission)
}

/// Only metadata is inspected. Unknown shapes fail closed: managed mode does
/// not follow an existing leaf symlink or write a multiply linked file. This is
/// a cooperative same-user check, not isolation from a hostile same-UID race.
#[cfg(target_os = "linux")]
fn disjoint_target(target: &std::path::Path, profile: &std::path::Path, runtime: &std::path::Path) -> Result<(), String> {
    use std::{fs, path::{Component, PathBuf}, os::unix::fs::MetadataExt};
    let denied = || MANAGED_UNSUPPORTED.to_owned();
    if !target.is_absolute() || target.components().any(|part| matches!(part, Component::ParentDir | Component::CurDir)) {
        return Err(denied());
    }
    match fs::symlink_metadata(target) {
        Ok(meta) if meta.file_type().is_symlink() || (meta.is_file() && meta.nlink() != 1)
            || (!meta.is_file() && !meta.is_dir()) => return Err(denied()),
        Ok(_) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => return Err(denied()),
    }
    let mut ancestor = target;
    let mut missing = Vec::new();
    let resolved: PathBuf = loop {
        match fs::canonicalize(ancestor) {
            Ok(mut existing) => {
                for leaf in missing.iter().rev() { existing.push(leaf); }
                break existing;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if missing.len() >= 128 { return Err(denied()); }
                missing.push(ancestor.file_name().ok_or_else(denied)?.to_os_string());
                ancestor = ancestor.parent().ok_or_else(denied)?;
            }
            Err(_) => return Err(denied()),
        }
    };
    for protected in [profile, runtime] {
        if resolved.starts_with(protected) || protected.starts_with(&resolved) {
            return Err(denied());
        }
        let protected_meta = fs::metadata(protected).map_err(|_| denied())?;
        let protected_key = (protected_meta.dev(), protected_meta.ino());
        // Physical directory aliases can have different canonical spellings.
        // Compare the protected root against each existing target ancestor.
        for ancestor in resolved.ancestors() {
            match fs::metadata(ancestor) {
                Ok(meta) if (meta.dev(), meta.ino()) == protected_key => return Err(denied()),
                Ok(_) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err(denied()),
            }
        }
        // A directory target may itself contain the protected root through an
        // alias. Compare only that target's identity, not all shared ancestors.
        if let Ok(target_meta) = fs::metadata(&resolved) {
            if target_meta.is_dir() {
                let target_key = (target_meta.dev(), target_meta.ino());
                for ancestor in protected.ancestors() {
                    let meta = fs::metadata(ancestor).map_err(|_| denied())?;
                    if (meta.dev(), meta.ino()) == target_key { return Err(denied()); }
                }
            }
        }
    }
    // Canonicalizing a leaf symlink would conceal its lexical placement, so
    // independently resolve its parent as well and check that full target.
    if let (Some(parent), Some(leaf)) = (target.parent(), target.file_name()) {
        if let Ok(parent) = fs::canonicalize(parent) {
            let lexical_target = parent.join(leaf);
            if [profile, runtime].iter().any(|root| lexical_target.starts_with(root) || root.starts_with(&lexical_target)) {
                return Err(denied());
            }
        }
    }
    Ok(())
}

/// Ordinary legacy writes share one physical runtime lease within the GUI
/// lifetime. Managed legacy writes are unsupported rather than implicitly
/// borrowing the typed service's writer capability.
pub(crate) struct LegacyRuntimeAdmission {
    session: Option<Arc<GuiProfileSession>>,
    #[cfg(target_os = "linux")]
    runtime: Option<Arc<ai_toolbox_management::owner::OrdinaryRuntimeLeaseGroup>>,
}

impl LegacyRuntimeAdmission {
    pub(crate) fn require_runtime(&self, path: &std::path::Path) -> Result<(), String> {
        self.ensure_current()?;
        #[cfg(target_os = "linux")]
        if let Some(runtime) = &self.runtime {
            runtime.require_runtime(path).map_err(|_| UNAVAILABLE.to_owned())?;
        }
        #[cfg(not(target_os = "linux"))]
        let _ = path;
        Ok(())
    }

    pub(crate) fn ensure_current(&self) -> Result<(), String> {
        if let Some(session) = &self.session { session.ensure_current()?; }
        #[cfg(target_os = "linux")]
        if let Some(runtime) = &self.runtime { runtime.ensure_current().map_err(|_| UNAVAILABLE.to_owned())?; }
        Ok(())
    }
}

/// Writer-only: an absent last path component may be bootstrapped as an empty
/// directory. Do not call from CLI/GUI read-only inspection or status paths.
pub(crate) fn admit_legacy_runtime(path: &std::path::Path) -> Result<LegacyRuntimeAdmission, String> {
    admit_legacy_runtime_with_session(current_session()?, path)
}

/// Carry the accepted session across an executor boundary. No production
/// caller may interpret a missing or expired session as ordinary permission.
pub(crate) fn admit_legacy_runtime_with_session(
    session: Option<Arc<GuiProfileSession>>, path: &std::path::Path,
) -> Result<LegacyRuntimeAdmission, String> {
    #[cfg(not(test))]
    if session.is_none() { return Err(UNAVAILABLE.to_owned()); }
    if let Some(session) = &session {
        session.ensure_current()?;
        if session.is_managed() { return Err(MANAGED_UNSUPPORTED.to_owned()); }
    }
    #[cfg(target_os = "linux")]
    let runtime = match &session {
        Some(session) => Some(session.ordinary_runtime(path)?),
        None => None, // Only existing cfg(test) helpers without GUI bootstrap.
    };
    #[cfg(not(target_os = "linux"))]
    let _ = path;
    let admission = LegacyRuntimeAdmission {
        session,
        #[cfg(target_os = "linux")]
        runtime,
    };
    admission.ensure_current()?;
    Ok(admission)
}

/// Resolve the current legacy runtime before a DB/master mutation. The accepted
/// session is moved into the real blocking lease acquisition, not looked up via
/// thread-local state inside a different executor thread.
pub(crate) async fn admit_legacy_codex(
    db: &crate::SqliteDbState,
) -> Result<LegacyRuntimeAdmission, String> {
    let session = reject_managed_operation()?;
    #[cfg(test)]
    if session.is_none() {
        return Ok(LegacyRuntimeAdmission { session, #[cfg(target_os = "linux")] runtime: None });
    }
    let root = crate::coding::runtime_location::get_codex_runtime_location_async(db).await?.host_path;
    admit_legacy_root_with_session(session, root).await
}

pub(crate) async fn admit_legacy_codex_root(
    root: std::path::PathBuf,
) -> Result<LegacyRuntimeAdmission, String> {
    let session = reject_managed_operation()?;
    admit_legacy_root_with_session(session, root).await
}

async fn admit_legacy_root_with_session(
    session: Option<Arc<GuiProfileSession>>, root: std::path::PathBuf,
) -> Result<LegacyRuntimeAdmission, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(session) = &session { session.ensure_current()?; }
        #[cfg(target_os = "linux")]
        let runtime = match &session {
            Some(session) => Some(session.ordinary_runtime(&root)?),
            None => None,
        };
        #[cfg(not(target_os = "linux"))]
        let _ = root;
        let admission = LegacyRuntimeAdmission { session, #[cfg(target_os = "linux")] runtime };
        admission.ensure_current()?;
        Ok(admission)
    }).await.map_err(|_| UNAVAILABLE.to_owned())?
}

#[cfg(all(test, target_os = "linux"))]
#[path = "management_admission_tests.rs"]
mod tests;

/// Managed views may read existing state but must not perform historical lazy
/// imports or self-healing writes. Missing/expired production session is error.
pub(crate) fn managed_read_only() -> Result<bool, String> {
    let session = current_session()?;
    if let Some(session) = session { session.ensure_current()?; return Ok(session.is_managed()); }
    Ok(false)
}
