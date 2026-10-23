//! Internal Linux ConfigOnly service. No CLI, GUI or transport entry is enabled.
//! All data paths are fixed by the acquired owner and existing master data.
use std::{fs, path::Path, os::unix::fs::MetadataExt};
use ai_toolbox_codex_config::plan::{plan_context_projection, ContextPatch, Target};
use ai_toolbox_storage::{SqliteDbState, SQLITE_DATABASE_FILE, codex_context_journal as journal};
use journal::{AdvanceResult, FailureCode, InsertResult, JournalEntry, JournalInput, Lookup, Phase};
use rusqlite::{Connection, Transaction};
use sha2::{Digest, Sha256};
use crate::{
    codex_master::{read_master, patch_master, MasterSnapshot, MasterError},
    config_file::{ConfigFile, ConfigFileErrorReason},
    owner::{OwnerPermit, ProfileIdentity, ProfileOwner, RuntimeResourceIdentity},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceError {
    Ownership,
    Storage,
    InvalidMaster,
    InvalidRequest,
    ProviderChanged,
    RevisionConflict,
    RuntimeUnsupported,
    FileUnavailable,
    JournalConflict,
    JournalInvalid,
    PersistenceUncertain,
    PhasePersistence { last_committed: Phase },
    Interrupted,
}

pub(crate) struct ContextRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub expected_provider_id: String,
    pub expected_revision: String,
    pub target: Target,
    pub patch: ContextPatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextStatus {
    pub phase: Phase,
    pub failure: Option<FailureCode>,
    pub replay: bool,
    pub context: Option<i64>,
    pub auto_compact: Option<i64>,
}
impl ContextStatus {
    fn from_entry(entry: JournalEntry, replay: bool) -> Self {
        Self {
            phase: if replay {
                entry.replay_phase()
            } else {
                entry.phase
            },
            failure: entry.failure,
            replay,
            context: entry.input.context,
            auto_compact: entry.input.auto_compact,
        }
    }
    // Reserved for the eventual typed adapter; partial states never mean success.
    pub(crate) fn exit_code(&self) -> i32 {
        if self.phase == Phase::Verified {
            0
        } else {
            3
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Checkpoint {
    None,
    #[cfg(test)]
    BeforeSaveCommit,
    #[cfg(test)]
    AfterSaved,
    #[cfg(test)]
    AfterStarted,
    #[cfg(test)]
    AfterWrite,
    #[cfg(test)]
    AfterProjected,
    #[cfg(test)]
    StartedCommitRollback,
    #[cfg(test)]
    VerifiedCommitRollback,
    #[cfg(test)]
    PreflightDrift,
    #[cfg(test)]
    NoopReadbackDrift,
    #[cfg(test)]
    ProjectedReadbackDrift,
    #[cfg(test)]
    PhaseWriteReadOnly,
    #[cfg(test)]
    ReplaceDrift,
}

// Declaration order drops this DB reference before this owner reference. The
// shared connection also retains the owner, including for external DB clones.
pub(crate) struct ContextService {
    db: SqliteDbState,
    owner: OwnerPermit,
    database_inode: (u64, u64),
}

fn master_error(error: MasterError) -> ServiceError {
    match error {
        MasterError::Storage => ServiceError::Storage,
        MasterError::Conflict => ServiceError::RevisionConflict,
        _ => ServiceError::InvalidMaster,
    }
}
fn journal_error(error: journal::JournalError) -> ServiceError {
    match error {
        journal::JournalError::Storage(_) => ServiceError::Storage,
        journal::JournalError::FingerprintConflict => ServiceError::JournalConflict,
        _ => ServiceError::JournalInvalid,
    }
}

fn valid_patch(patch: ContextPatch) -> bool {
    let values = [
        patch.model_context_window,
        patch.model_auto_compact_token_limit,
    ];
    values.iter().any(Option::is_some)
        && values
            .into_iter()
            .flatten()
            .all(|v| v > 0 && v <= i64::MAX as u64)
        && !matches!((patch.model_context_window,patch.model_auto_compact_token_limit),(Some(c),Some(a)) if a>c)
}

// Constructed only after a new Advanced start and its successful commit.
// Replay cannot produce this non-cloneable private capability.
struct CommittedStart;
fn project(
    _start: CommittedStart,
    file: ConfigFile<'_, '_>,
    candidate: &[u8],
) -> Result<Vec<u8>, crate::config_file::ConfigFileError> {
    file.replace(candidate)
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn digest(parts: &[&[u8]]) -> String {
    let mut h = Sha256::new();
    for part in parts {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    format!("{:x}", h.finalize())
}
fn database_identity(path: &Path) -> Result<(u64, u64), ServiceError> {
    let m = fs::symlink_metadata(path).map_err(|_| ServiceError::Storage)?;
    let uid = crate::owner::linux::current_uid().map_err(|_| ServiceError::Ownership)?;
    if !m.is_file() || m.uid() != uid || m.nlink() != 1 || m.mode() & 0o022 != 0 {
        return Err(ServiceError::Storage);
    }
    Ok((m.dev(), m.ino()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DatabaseIdentity {
    main: (u64, u64),
    sidecars: [Option<(u64, u64)>; 3],
}

impl DatabaseIdentity {
    fn existing(path: &Path) -> Result<Self, ServiceError> {
        let main = database_identity(path)?;
        let mut sidecars = [None; 3];
        for (slot, suffix) in sidecars.iter_mut().zip(["-wal", "-shm", "-journal"]) {
            let side = path.with_file_name(format!("{SQLITE_DATABASE_FILE}{suffix}"));
            match fs::symlink_metadata(&side) {
                Ok(_) => *slot = Some(database_identity(&side)?),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err(ServiceError::Storage),
            }
        }
        Ok(Self { main, sidecars })
    }

    fn validate_connection(
        owner: &OwnerPermit,
        db: &SqliteDbState,
        connection: &Connection,
    ) -> Result<Self, ServiceError> {
        owner.ensure_current().map_err(|_| ServiceError::Ownership)?;
        let expected = owner.profile().canonical_path().join(SQLITE_DATABASE_FILE);
        let before = Self::existing(&expected)?;
        // db_path is cached input, not evidence of the connection's current
        // main database. Validate both it and SQLite's own main filename.
        let actual = Path::new(connection.path().ok_or(ServiceError::Storage)?);
        for path in [db.db_path(), actual] {
            if !path.is_absolute()
                || fs::canonicalize(path).map_err(|_| ServiceError::Storage)? != expected
                || database_identity(path)? != before.main
            {
                return Err(ServiceError::Storage);
            }
        }
        let mut moved: std::ffi::c_int = 1;
        // SAFETY: The storage callback holds its mutex for this entire
        // validation. The live handle is borrowed, the fixed main name is NUL
        // terminated, and HAS_MOVED writes exactly one c_int. No pointer or
        // callback escapes. Unsupported VFS implementations fail closed.
        let result = unsafe {
            rusqlite::ffi::sqlite3_file_control(
                connection.handle(),
                b"main\0".as_ptr().cast(),
                rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
                (&mut moved as *mut std::ffi::c_int).cast(),
            )
        };
        if result != rusqlite::ffi::SQLITE_OK || moved != 0 {
            return Err(ServiceError::Storage);
        }
        owner.ensure_current().map_err(|_| ServiceError::Ownership)?;
        if Self::existing(&expected)? != before {
            return Err(ServiceError::Storage);
        }
        Ok(before)
    }
}

impl ContextService {
    /// Writer initialization only, not a read-command implementation. Requires
    /// an existing file database; shared storage initialization may seed/migrate.
    pub(crate) fn open(profile: &Path) -> Result<Self, ServiceError> {
        let identity = ProfileIdentity::existing(profile).map_err(|_| ServiceError::Ownership)?;
        let owner = ProfileOwner::acquire(identity).map_err(|_| ServiceError::Ownership)?;
        let path = owner.profile().canonical_path().join(SQLITE_DATABASE_FILE);
        let before = DatabaseIdentity::existing(&path)?;
        owner
            .ensure_current()
            .map_err(|_| ServiceError::Ownership)?;
        let db = SqliteDbState::open(path.clone()).map_err(|_| ServiceError::Storage)?;
        if database_identity(&path)? != before.main {
            return Err(ServiceError::Storage);
        }
        Self::attach(&owner, &db)
    }

    /// Attach only to an existing owner's already initialized shared state.
    /// No owner acquisition, database open, schema work or seed is performed.
    /// This cannot retroactively prove owner-before-open for the caller; that
    /// startup ordering and the full writer admission map remain prerequisites.
    /// Callers must honor storage's retained-connection no-replacement contract;
    /// same-path/inode checks cannot identify a reopened connection as original.
    pub(crate) fn attach(owner: &OwnerPermit, db: &SqliteDbState) -> Result<Self, ServiceError> {
        let database_inode = db
            .retain_lifetime_owner_checked(owner.lifetime_owner(), |connection| {
                DatabaseIdentity::validate_connection(owner, db, connection).map(|identity| identity.main)
            })
            .map_err(|error| match error {
                ai_toolbox_storage::sqlite_state::LifetimeOwnerError::Unavailable => ServiceError::Storage,
                ai_toolbox_storage::sqlite_state::LifetimeOwnerError::DifferentOwner => ServiceError::Ownership,
                ai_toolbox_storage::sqlite_state::LifetimeOwnerError::Validation(error) => error,
            })?;
        Ok(Self {
            db: db.clone(),
            owner: owner.clone(),
            database_inode,
        })
    }
    fn ensure_current(&self) -> Result<(), ServiceError> {
        self.db.with_conn(|connection| Ok(self.validate_current_connection(connection)))
            .map_err(|_| ServiceError::Storage)?
    }
    fn validate_current_connection(&self, connection: &Connection) -> Result<(), ServiceError> {
        let current = DatabaseIdentity::validate_connection(&self.owner, &self.db, connection)?;
        // SQLite may legitimately create/remove sidecars between operations.
        // Their safety and single-validation stability are checked above;
        // only the main DB inode is pinned for this service's lifetime.
        if current.main != self.database_inode {
            return Err(ServiceError::Storage);
        }
        Ok(())
    }
    fn master(&self) -> Result<MasterSnapshot, ServiceError> {
        self.db
            .with_conn(|c| Ok(self.validate_current_connection(c)
                .and_then(|_| read_master(c).map_err(master_error))))
            .map_err(|_| ServiceError::Storage)?
    }
    fn runtime(master: &MasterSnapshot) -> Result<RuntimeResourceIdentity, ServiceError> {
        RuntimeResourceIdentity::existing(
            master
                .explicit_root()
                .map_err(|_| ServiceError::RuntimeUnsupported)?,
        )
        .map_err(|_| ServiceError::RuntimeUnsupported)
    }
    fn revision(
        &self,
        master: &MasterSnapshot,
        runtime: &RuntimeResourceIdentity,
        live: &[u8],
    ) -> Result<String, ServiceError> {
        let key = runtime.key();
        let p = self.owner.profile().key();
        Ok(digest(&[
            b"codex-context-revision-v1",
            &p.device.to_le_bytes(),
            &p.inode.to_le_bytes(),
            &key.device.to_le_bytes(),
            &key.inode.to_le_bytes(),
            &master
                .revision_bytes()
                .map_err(|_| ServiceError::InvalidMaster)?,
            live,
        ]))
    }
    /// Reads only through an already initialized owner; does not create a new
    /// profile, database or default provider. Runtime lease is an internal effect.
    pub(crate) fn current_revision(&self) -> Result<(String, String), ServiceError> {
        self.ensure_current()?;
        let writer = self
            .owner
            .try_writer()
            .map_err(|_| ServiceError::Ownership)?;
        let master = self.master()?;
        let runtime = Self::runtime(&master)?;
        let resources = writer
            .acquire_resources(vec![runtime.clone()])
            .map_err(|_| ServiceError::Ownership)?;
        let file = ConfigFile::read(&writer, &resources, &runtime)
            .map_err(|_| ServiceError::FileUnavailable)?;
        Ok((
            self.revision(&master, &runtime, file.bytes())?,
            master.provider_id,
        ))
    }
    fn fingerprint(&self, r: &ContextRequest) -> String {
        let key = self.owner.profile().key();
        let target = match r.target {
            Target::Common => b"common".as_slice(),
            Target::Provider => b"provider".as_slice(),
        };
        let patch = serde_json::to_vec(&(
            r.patch.model_context_window,
            r.patch.model_auto_compact_token_limit,
        ))
        .expect("integer options serialize");
        digest(&[
            b"codex-context-config-only-v1",
            &key.device.to_le_bytes(),
            &key.inode.to_le_bytes(),
            r.request_id.as_bytes(),
            r.expected_provider_id.as_bytes(),
            r.expected_revision.as_bytes(),
            target,
            &patch,
        ])
    }
    fn lookup(&self, r: &ContextRequest, fingerprint: &str) -> Result<Lookup, ServiceError> {
        self.db
            .with_conn(|c| Ok(self.validate_current_connection(c)
                .and_then(|_| journal::lookup(c, &r.request_id, fingerprint).map_err(journal_error))))
            .map_err(|_| ServiceError::Storage)?
    }
    /// Only successful typed operations commit. In particular never pass
    /// Ok(Err(..)) to helpers::db_transaction, whose Ok branch commits.
    fn transaction<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, ServiceError>,
    ) -> Result<T, ServiceError> {
        self.db
            .with_conn_mut(|c| {
                let outcome = (|| {
                    self.validate_current_connection(c)?;
                    let tx = c.transaction().map_err(|_| ServiceError::Storage)?;
                    let result = operation(&tx)?;
                    tx.commit()
                        .map_err(|_| ServiceError::PersistenceUncertain)?;
                    Ok(result)
                })();
                Ok(outcome)
            })
            .map_err(|_| ServiceError::Storage)?
    }
    fn advance(
        &self,
        r: &ContextRequest,
        fp: &str,
        from: Phase,
        to: Phase,
        failure: Option<FailureCode>,
    ) -> Result<JournalEntry, ServiceError> {
        self.advance_at(r, fp, from, to, failure, Checkpoint::None)
    }
    fn advance_at(
        &self,
        r: &ContextRequest,
        fp: &str,
        from: Phase,
        to: Phase,
        failure: Option<FailureCode>,
        checkpoint: Checkpoint,
    ) -> Result<JournalEntry, ServiceError> {
        self.transaction(|tx| {
            let entry = match journal::advance(tx, &r.request_id, fp, from, to, failure)
                .map_err(journal_error)?
            {
                AdvanceResult::Advanced(entry) => entry,
                AdvanceResult::Replay(_) => return Err(ServiceError::JournalConflict),
            };
            // Synthetic pre-commit transaction errors, not real COMMIT/I/O faults.
            #[cfg(test)]
            if (to == Phase::ProjectionStarted && checkpoint == Checkpoint::StartedCommitRollback)
                || (to == Phase::Verified && checkpoint == Checkpoint::VerifiedCommitRollback)
            {
                return Err(ServiceError::Interrupted);
            }
            Ok(entry)
        })
        .map_err(|error| match error {
            ServiceError::Storage => ServiceError::PhasePersistence {
                last_committed: from,
            },
            other => other,
        })
    }
    pub(crate) fn execute(&self, r: &ContextRequest) -> Result<ContextStatus, ServiceError> {
        self.execute_at(r, Checkpoint::None)
    }
    fn execute_at(
        &self,
        r: &ContextRequest,
        checkpoint: Checkpoint,
    ) -> Result<ContextStatus, ServiceError> {
        if r.schema_version != 1
            || !valid_patch(r.patch)
            || !valid_id(&r.request_id)
            || !valid_id(&r.expected_provider_id)
            || r.expected_revision.len() != 64
            || !r
                .expected_revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ServiceError::InvalidRequest);
        }
        self.ensure_current()?;
        let writer = self
            .owner
            .try_writer()
            .map_err(|_| ServiceError::Ownership)?;
        let fp = self.fingerprint(r);
        // Replay precedes provider/runtime/revision inspection, including after
        // this same operation changed the master data or the live file.
        if let Lookup::Replay(entry) = self.lookup(r, &fp)? {
            return Ok(ContextStatus::from_entry(entry, true));
        }
        let master = self.master()?;
        if master.provider_id != r.expected_provider_id {
            return Err(ServiceError::ProviderChanged);
        }
        let runtime = Self::runtime(&master)?;
        let resources = writer
            .acquire_resources(vec![runtime.clone()])
            .map_err(|_| ServiceError::Ownership)?;
        let file = ConfigFile::read(&writer, &resources, &runtime)
            .map_err(|_| ServiceError::FileUnavailable)?;
        if self.revision(&master, &runtime, file.bytes())? != r.expected_revision {
            return Err(ServiceError::RevisionConflict);
        }
        let live = std::str::from_utf8(file.bytes()).map_err(|_| ServiceError::FileUnavailable)?;
        let plan = plan_context_projection(
            live,
            &master.common_config,
            &master.provider_config,
            r.target,
            r.patch,
        )
        .map_err(|_| ServiceError::InvalidRequest)?;
        let input = JournalInput {
            request_id: r.request_id.clone(),
            provider_id: master.provider_id.clone(),
            request_fingerprint: fp.clone(),
            expected_revision: r.expected_revision.clone(),
            planned_live_digest: digest(&[plan.live_candidate.as_bytes()]),
            context: plan.master.context.value,
            auto_compact: plan.master.auto_compact.value,
            master_changed: plan.master.changed,
            live_changed: plan.live_changed,
        };
        let saved = self.transaction(|tx| {
            if !matches!(
                journal::lookup(tx, &r.request_id, &fp).map_err(journal_error)?,
                Lookup::Missing
            ) {
                return Err(ServiceError::JournalConflict);
            }
            patch_master(tx, &master, r.target, &plan.master.candidate).map_err(master_error)?;
            let entry = match journal::insert_saved(tx, input).map_err(journal_error)? {
                InsertResult::Inserted(entry) => entry,
                InsertResult::Replay(_) => return Err(ServiceError::JournalConflict),
            };
            #[cfg(test)]
            if checkpoint == Checkpoint::BeforeSaveCommit {
                return Err(ServiceError::Interrupted);
            }
            Ok(entry)
        })?;
        #[cfg(test)]
        if checkpoint == Checkpoint::AfterSaved {
            return Err(ServiceError::Interrupted);
        }
        #[cfg(test)]
        if checkpoint == Checkpoint::PreflightDrift {
            fs::write(
                runtime.canonical_path().join("config.toml"),
                b"# synthetic external drift\n",
            )
            .unwrap();
        }
        if file.preflight().is_err() {
            let entry = self.advance(
                r,
                &fp,
                Phase::SavedOnly,
                Phase::SavedOnly,
                Some(FailureCode::ProjectionPreflightFailed),
            )?;
            return Ok(ContextStatus::from_entry(entry, false));
        }
        if !plan.live_changed {
            #[cfg(test)]
            if checkpoint == Checkpoint::NoopReadbackDrift {
                fs::write(
                    runtime.canonical_path().join("config.toml"),
                    b"# synthetic external drift\n",
                )
                .unwrap();
            }
            let result = file.readback();
            let (next, failure) = match result {
                Ok(bytes) if digest(&[&bytes]) == saved.input.planned_live_digest => {
                    (Phase::Verified, None)
                }
                Ok(_) => (Phase::SavedOnly, Some(FailureCode::ReadbackMismatch)),
                Err(error) => (
                    Phase::SavedOnly,
                    Some(match error.reason() {
                        ConfigFileErrorReason::ConfigChanged
                        | ConfigFileErrorReason::ReadbackMismatch => FailureCode::ReadbackMismatch,
                        _ => FailureCode::ReadbackFailed,
                    }),
                ),
            };
            return self
                .advance(r, &fp, Phase::SavedOnly, next, failure)
                .map(|e| ContextStatus::from_entry(e, false));
        }
        #[cfg(test)]
        if checkpoint == Checkpoint::PhaseWriteReadOnly {
            self.db
                .with_conn(|c| {
                    c.execute_batch("PRAGMA query_only = ON")
                        .map_err(|_| "synthetic fixture".into())
                })
                .unwrap();
        }
        self.advance_at(
            r,
            &fp,
            Phase::SavedOnly,
            Phase::ProjectionStarted,
            None,
            checkpoint,
        )?;
        let committed_start = CommittedStart;
        #[cfg(test)]
        if checkpoint == Checkpoint::AfterStarted {
            return Err(ServiceError::Interrupted);
        }
        #[cfg(test)]
        if checkpoint == Checkpoint::ReplaceDrift {
            fs::write(
                runtime.canonical_path().join("config.toml"),
                b"# synthetic external drift\n",
            )
            .unwrap();
        }
        // Any failure after the durable start is conservatively uncertain; no
        // automatic retry or rollback can assume rename did not happen.
        // A trusted caller can replace a Connection through with_conn_mut.
        // Recheck under that same mutex before the file effect as well, without
        // allowing such a replacement between validation and publication.
        let projection = self.db.with_conn(|c| {
            Ok(self.validate_current_connection(c).map(|_| {
                project(committed_start, file, plan.live_candidate.as_bytes())
            }))
        }).map_err(|_| ServiceError::Storage)??;
        match projection {
            Ok(_) => (),
            Err(_) => {
                return self
                    .advance(
                        r,
                        &fp,
                        Phase::ProjectionStarted,
                        Phase::ProjectionUncertain,
                        Some(FailureCode::ProjectionOutcomeUnknown),
                    )
                    .map(|e| ContextStatus::from_entry(e, false))
            }
        };
        #[cfg(test)]
        if checkpoint == Checkpoint::AfterWrite {
            return Err(ServiceError::Interrupted);
        }
        self.advance(
            r,
            &fp,
            Phase::ProjectionStarted,
            Phase::ProjectedUnverified,
            None,
        )?;
        #[cfg(test)]
        if checkpoint == Checkpoint::AfterProjected {
            return Err(ServiceError::Interrupted);
        }
        #[cfg(test)]
        if checkpoint == Checkpoint::ProjectedReadbackDrift {
            fs::write(
                runtime.canonical_path().join("config.toml"),
                b"# synthetic external drift\n",
            )
            .unwrap();
        }
        let (phase, failure) = match ConfigFile::read(&writer, &resources, &runtime) {
            Ok(current) if digest(&[current.bytes()]) == saved.input.planned_live_digest => {
                (Phase::Verified, None)
            }
            Ok(_) => (
                Phase::ProjectedUnverified,
                Some(FailureCode::ReadbackMismatch),
            ),
            Err(_) => (
                Phase::ProjectedUnverified,
                Some(FailureCode::ReadbackFailed),
            ),
        };
        self.advance_at(
            r,
            &fp,
            Phase::ProjectedUnverified,
            phase,
            failure,
            checkpoint,
        )
        .map(|e| ContextStatus::from_entry(e, false))
    }
}

#[cfg(test)]
#[path = "codex_service/attach_tests.rs"]
mod attach_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use ai_toolbox_storage::{
        helpers::{db_get, db_put},
        schema::DbTable,
    };
    use serde_json::json;
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};

    struct Fixture {
        _root: tempfile::TempDir,
        profile: PathBuf,
        runtime: PathBuf,
        auth_inode: u64,
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let profile = root.path().join("profile");
            let runtime = root.path().join("codex");
            for p in [&profile, &runtime] {
                fs::create_dir(p).unwrap();
                fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
            }
            fs::write(runtime.join("config.toml"),concat!("# synthetic config\nmodel_context_window = 1000000\n",
                "model_auto_compact_token_limit = 900000\nmodel_provider = \"synthetic\"\n",
                "requires_openai_auth = true\n[model_providers.synthetic]\nbase_url = \"https://invalid.example\"\n")).unwrap();
            fs::write(
                runtime.join("auth.json"),
                b"{\"synthetic\":\"not-a-credential\"}\n",
            )
            .unwrap();
            let db = SqliteDbState::open(profile.join(SQLITE_DATABASE_FILE)).unwrap();
            db.with_conn(|c| {
                db_put(c,DbTable::CodexCommonConfig,"common",&json!({"config":"model_context_window = 1000000\n",
                    "root_dir":runtime.to_str().unwrap(),"unknown":{"retained":true}}))?;
                db_put(c,DbTable::CodexProvider,"synthetic-provider",&json!({"is_applied":true,"is_disabled":false,
                    "unknown_outer":{"retained":7},"settings_config":json!({"config":"model_auto_compact_token_limit = 900000\n",
                        "auth":{"synthetic":"not-a-credential"},"models":[{"id":"synthetic-model"}],"unknown":{"keep":true}}).to_string()}))
            }).unwrap();
            drop(db);
            for path in [
                runtime.join("config.toml"),
                runtime.join("auth.json"),
                profile.join(SQLITE_DATABASE_FILE),
            ] {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
            }
            let auth_inode = fs::metadata(runtime.join("auth.json")).unwrap().ino();
            Self {
                _root: root,
                profile,
                runtime,
                auth_inode,
            }
        }
        fn open(&self) -> ContextService {
            ContextService::open(&self.profile).unwrap()
        }
        fn auth_unchanged(&self) {
            assert_eq!(
                fs::read(self.runtime.join("auth.json")).unwrap(),
                b"{\"synthetic\":\"not-a-credential\"}\n"
            );
            assert_eq!(
                fs::metadata(self.runtime.join("auth.json")).unwrap().ino(),
                self.auth_inode
            );
        }
        fn live(&self) -> Vec<u8> {
            fs::read(self.runtime.join("config.toml")).unwrap()
        }
    }
    fn request(service: &ContextService, id: &str, target: Target) -> ContextRequest {
        let (revision, provider) = service.current_revision().unwrap();
        ContextRequest {
            schema_version: 1,
            request_id: id.into(),
            expected_provider_id: provider,
            expected_revision: revision,
            target,
            patch: ContextPatch {
                model_context_window: Some(2000000),
                model_auto_compact_token_limit: None,
            },
        }
    }
    fn records(service: &ContextService) -> (serde_json::Value, serde_json::Value) {
        service
            .db
            .with_conn(|c| {
                Ok((
                    db_get(c, DbTable::CodexCommonConfig, "common")?.unwrap(),
                    db_get(c, DbTable::CodexProvider, "synthetic-provider")?.unwrap(),
                ))
            })
            .unwrap()
    }
    fn journal_entry(service: &ContextService, r: &ContextRequest) -> JournalEntry {
        let Lookup::Replay(e) = service.lookup(r, &service.fingerprint(r)).unwrap() else {
            panic!("missing fixture journal")
        };
        e
    }
    #[test]
    fn disk_common_roundtrip_reopens_and_preserves_auth_unknowns_and_unrequested_fields() {
        let f = Fixture::new();
        let service = f.open();
        let before = records(&service);
        let r = request(&service, "common-roundtrip", Target::Common);
        let status = service.execute(&r).unwrap();
        assert_eq!(status.phase, Phase::Verified);
        assert_eq!(status.exit_code(), 0);
        let after = records(&service);
        assert_eq!(before.1, after.1);
        assert_eq!(before.0["unknown"], after.0["unknown"]);
        assert_eq!(before.0["root_dir"], after.0["root_dir"]);
        assert!(after.0["config"].as_str().unwrap().contains("2000000"));
        let live = String::from_utf8(f.live()).unwrap();
        assert!(live.contains("model_context_window = 2000000"));
        for kept in [
            "# synthetic config",
            "model_auto_compact_token_limit = 900000",
            "requires_openai_auth = true",
            "base_url = \"https://invalid.example\"",
        ] {
            assert!(live.contains(kept));
        }
        f.auth_unchanged();
        drop(service);
        let reopened = f.open();
        assert_eq!(records(&reopened), after);
        assert_eq!(journal_entry(&reopened, &r).phase, Phase::Verified);
        assert!(reopened.execute(&r).unwrap().replay);
        f.auth_unchanged();
    }
    #[test]
    fn disk_provider_roundtrip_keeps_settings_string_auth_models_unknown_values() {
        let f = Fixture::new();
        let service = f.open();
        let before = records(&service);
        let mut r = request(&service, "provider-roundtrip", Target::Provider);
        r.patch = ContextPatch {
            model_context_window: None,
            model_auto_compact_token_limit: Some(800000),
        };
        assert_eq!(service.execute(&r).unwrap().phase, Phase::Verified);
        let after = records(&service);
        assert_eq!(before.0, after.0);
        let old: serde_json::Value =
            serde_json::from_str(before.1["settings_config"].as_str().unwrap()).unwrap();
        let new: serde_json::Value =
            serde_json::from_str(after.1["settings_config"].as_str().unwrap()).unwrap();
        for name in ["auth", "models", "unknown"] {
            assert_eq!(old[name], new[name]);
        }
        assert_eq!(before.1["unknown_outer"], after.1["unknown_outer"]);
        assert!(new["config"].as_str().unwrap().contains("800000"));
        drop(service);
        let reopened = f.open();
        assert_eq!(records(&reopened), after);
        f.auth_unchanged();
    }
    #[test]
    fn disk_replay_after_new_epoch_does_not_replace_config_again() {
        let f = Fixture::new();
        let service = f.open();
        let r = request(&service, "replay", Target::Common);
        let epoch = service.owner.epoch();
        service.execute(&r).unwrap();
        let inode = fs::metadata(f.runtime.join("config.toml")).unwrap().ino();
        let bytes = f.live();
        drop(service);
        let reopened = f.open();
        assert_ne!(reopened.owner.epoch(), epoch);
        assert!(reopened.execute(&r).unwrap().replay);
        assert_eq!(f.live(), bytes);
        assert_eq!(
            fs::metadata(f.runtime.join("config.toml")).unwrap().ino(),
            inode
        );
        f.auth_unchanged();
    }
    #[test]
    fn disk_request_fingerprint_and_stale_revision_reject_without_new_effects() {
        let f = Fixture::new();
        let service = f.open();
        let mut r = request(&service, "first", Target::Common);
        service.execute(&r).unwrap();
        let before = records(&service);
        let live = f.live();
        r.patch.model_context_window = Some(3000000);
        assert_eq!(service.execute(&r), Err(ServiceError::JournalConflict));
        r.request_id = "new-request".into();
        assert_eq!(service.execute(&r), Err(ServiceError::RevisionConflict));
        assert!(matches!(
            service.lookup(&r, &service.fingerprint(&r)).unwrap(),
            Lookup::Missing
        ));
        assert_eq!(records(&service), before);
        assert_eq!(f.live(), live);
        f.auth_unchanged();
    }
    #[test]
    fn disk_synthetic_save_transaction_error_rolls_back_master_and_journal() {
        let f = Fixture::new();
        let service = f.open();
        let before = records(&service);
        let live = f.live();
        let r = request(&service, "rollback", Target::Common);
        assert_eq!(
            service.execute_at(&r, Checkpoint::BeforeSaveCommit),
            Err(ServiceError::Interrupted)
        );
        assert_eq!(records(&service), before);
        assert_eq!(f.live(), live);
        assert!(matches!(
            service.lookup(&r, &service.fingerprint(&r)).unwrap(),
            Lookup::Missing
        ));
        drop(service);
        let reopened = f.open();
        assert_eq!(records(&reopened), before);
        f.auth_unchanged();
    }
    #[test]
    fn disk_saved_only_replay_never_implicitly_resumes_new_request_can_recover() {
        let f = Fixture::new();
        let service = f.open();
        let live = f.live();
        let r = request(&service, "saved-only", Target::Common);
        assert_eq!(
            service.execute_at(&r, Checkpoint::AfterSaved),
            Err(ServiceError::Interrupted)
        );
        assert_eq!(f.live(), live);
        drop(service);
        let reopened = f.open();
        let status = reopened.execute(&r).unwrap();
        assert_eq!(status.phase, Phase::SavedOnly);
        assert_eq!(status.exit_code(), 3);
        assert!(status.replay);
        assert_eq!(f.live(), live);
        let recovery = request(&reopened, "saved-recovery", Target::Common);
        assert_eq!(reopened.execute(&recovery).unwrap().phase, Phase::Verified);
        assert!(!journal_entry(&reopened, &recovery).input.master_changed);
        f.auth_unchanged();
    }
    #[test]
    fn disk_interruption_after_started_before_rename_replays_uncertain_without_write() {
        let f = Fixture::new();
        let service = f.open();
        let live = f.live();
        let r = request(&service, "started", Target::Common);
        assert_eq!(
            service.execute_at(&r, Checkpoint::AfterStarted),
            Err(ServiceError::Interrupted)
        );
        assert_eq!(f.live(), live);
        drop(service);
        let reopened = f.open();
        let status = reopened.execute(&r).unwrap();
        assert_eq!(status.phase, Phase::ProjectionUncertain);
        assert_eq!(status.exit_code(), 3);
        assert_eq!(f.live(), live);
        f.auth_unchanged();
    }
    #[test]
    fn disk_interruption_after_real_rename_keeps_started_and_does_not_rewrite_on_reopen() {
        let f = Fixture::new();
        let service = f.open();
        let old = f.live();
        let r = request(&service, "renamed", Target::Common);
        assert_eq!(
            service.execute_at(&r, Checkpoint::AfterWrite),
            Err(ServiceError::Interrupted)
        );
        assert_ne!(f.live(), old);
        let bytes = f.live();
        let inode = fs::metadata(f.runtime.join("config.toml")).unwrap().ino();
        drop(service);
        let reopened = f.open();
        assert_eq!(
            reopened.execute(&r).unwrap().phase,
            Phase::ProjectionUncertain
        );
        assert_eq!(f.live(), bytes);
        assert_eq!(
            fs::metadata(f.runtime.join("config.toml")).unwrap().ino(),
            inode
        );
        f.auth_unchanged();
    }
    #[test]
    fn disk_projected_unverified_reopens_without_claiming_verified() {
        let f = Fixture::new();
        let service = f.open();
        let r = request(&service, "unverified", Target::Common);
        assert_eq!(
            service.execute_at(&r, Checkpoint::AfterProjected),
            Err(ServiceError::Interrupted)
        );
        let live = f.live();
        drop(service);
        let reopened = f.open();
        let status = reopened.execute(&r).unwrap();
        assert_eq!(status.phase, Phase::ProjectedUnverified);
        assert_eq!(status.exit_code(), 3);
        assert_eq!(f.live(), live);
        let recovery = request(&reopened, "verify-new-id", Target::Common);
        assert_eq!(reopened.execute(&recovery).unwrap().phase, Phase::Verified);
        assert!(!journal_entry(&reopened, &recovery).input.live_changed);
        f.auth_unchanged();
    }
    #[test]
    fn disk_noop_verifies_without_replacing_file() {
        let f = Fixture::new();
        let service = f.open();
        let mut r = request(&service, "noop", Target::Common);
        r.patch.model_context_window = Some(1000000);
        let inode = fs::metadata(f.runtime.join("config.toml")).unwrap().ino();
        let bytes = f.live();
        assert_eq!(service.execute(&r).unwrap().phase, Phase::Verified);
        let e = journal_entry(&service, &r);
        assert!(!e.input.master_changed && !e.input.live_changed);
        assert_eq!(f.live(), bytes);
        assert_eq!(
            fs::metadata(f.runtime.join("config.toml")).unwrap().ino(),
            inode
        );
        f.auth_unchanged();
    }
    #[test]
    fn disk_busy_writer_invalid_id_provider_and_unsafe_leaf_fail_before_master_change() {
        let f = Fixture::new();
        let service = f.open();
        let mut r = request(&service, "controls", Target::Common);
        let before = records(&service);
        let writer = service.owner.try_writer().unwrap();
        assert_eq!(service.execute(&r), Err(ServiceError::Ownership));
        drop(writer);
        r.request_id = "invalid/id".into();
        assert_eq!(service.execute(&r), Err(ServiceError::InvalidRequest));
        r.request_id = "controls".into();
        r.expected_provider_id = "different-provider".into();
        assert_eq!(service.execute(&r), Err(ServiceError::ProviderChanged));
        r.expected_provider_id = "synthetic-provider".into();
        let original = f.runtime.join("config.toml");
        fs::rename(&original, f.runtime.join("config-saved")).unwrap();
        std::os::unix::fs::symlink("config-saved", &original).unwrap();
        assert_eq!(service.execute(&r), Err(ServiceError::FileUnavailable));
        assert_eq!(records(&service), before);
        f.auth_unchanged();
    }
    #[test]
    fn disk_runtime_lease_conflict_keeps_master_and_live_unchanged() {
        let f = Fixture::new();
        let service = f.open();
        let r = request(&service, "resource-busy", Target::Common);
        let before = records(&service);
        let live = f.live();
        let other = f._root.path().join("other-profile");
        fs::create_dir(&other).unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
        let owner = ProfileOwner::acquire(ProfileIdentity::existing(other).unwrap()).unwrap();
        let writer = owner.try_writer().unwrap();
        let _held = writer
            .acquire_resources(vec![RuntimeResourceIdentity::existing(&f.runtime).unwrap()])
            .unwrap();
        assert_eq!(service.execute(&r), Err(ServiceError::Ownership));
        assert_eq!(records(&service), before);
        assert_eq!(f.live(), live);
        f.auth_unchanged();
    }
    #[test]
    fn disk_real_commit_veto_never_reports_uncommitted_phase_as_success() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        for nth in 1..=4 {
            let f = Fixture::new();
            let service = f.open();
            let r = request(&service, "commit-veto", Target::Common);
            let before = records(&service);
            let old_live = f.live();
            let count = Arc::new(AtomicUsize::new(0));
            let seen = count.clone();
            service
                .db
                .with_conn(|c| {
                    c.commit_hook(Some(move || seen.fetch_add(1, Ordering::SeqCst) + 1 == nth))
                        .map_err(|_| "synthetic hook installation".to_string())?;
                    Ok(())
                })
                .unwrap();
            assert_eq!(service.execute(&r), Err(ServiceError::PersistenceUncertain));
            assert_eq!(count.load(Ordering::SeqCst), nth);
            drop(service);
            let reopened = f.open();
            if nth == 1 {
                assert_eq!(records(&reopened), before);
                assert_eq!(f.live(), old_live);
                assert!(matches!(
                    reopened.lookup(&r, &reopened.fingerprint(&r)).unwrap(),
                    Lookup::Missing
                ));
            } else {
                let status = reopened.execute(&r).unwrap();
                assert!(status.replay);
                assert_eq!(status.exit_code(), 3);
                assert_eq!(
                    status.phase,
                    match nth {
                        2 => Phase::SavedOnly,
                        3 => Phase::ProjectionUncertain,
                        _ => Phase::ProjectedUnverified,
                    }
                );
                if nth == 2 {
                    assert_eq!(f.live(), old_live);
                } else {
                    assert_ne!(f.live(), old_live);
                }
            }
            f.auth_unchanged();
        }
    }
    #[test]
    fn disk_phase_sqlite_write_failure_preserves_saved_phase_not_fake_conflict() {
        let f = Fixture::new();
        let service = f.open();
        let r = request(&service, "phase-readonly", Target::Common);
        let live = f.live();
        assert_eq!(
            service.execute_at(&r, Checkpoint::PhaseWriteReadOnly),
            Err(ServiceError::PhasePersistence {
                last_committed: Phase::SavedOnly
            })
        );
        assert_eq!(f.live(), live);
        drop(service);
        let reopened = f.open();
        assert_eq!(reopened.execute(&r).unwrap().phase, Phase::SavedOnly);
        f.auth_unchanged();
    }
    #[test]
    fn disk_preflight_and_noop_or_projected_readback_drift_keep_partial_status() {
        for checkpoint in [
            Checkpoint::PreflightDrift,
            Checkpoint::NoopReadbackDrift,
            Checkpoint::ProjectedReadbackDrift,
        ] {
            let f = Fixture::new();
            let service = f.open();
            let mut r = request(&service, "drift", Target::Common);
            if checkpoint == Checkpoint::NoopReadbackDrift {
                r.patch.model_context_window = Some(1000000);
            }
            let status = service.execute_at(&r, checkpoint).unwrap();
            assert_eq!(status.exit_code(), 3);
            let (phase, failure) = match checkpoint {
                Checkpoint::PreflightDrift => {
                    (Phase::SavedOnly, FailureCode::ProjectionPreflightFailed)
                }
                Checkpoint::NoopReadbackDrift => (Phase::SavedOnly, FailureCode::ReadbackMismatch),
                _ => (Phase::ProjectedUnverified, FailureCode::ReadbackMismatch),
            };
            assert_eq!(status.phase, phase);
            assert_eq!(status.failure, Some(failure));
            drop(service);
            let reopened = f.open();
            assert_eq!(reopened.execute(&r).unwrap().phase, phase);
            f.auth_unchanged();
        }
    }
    #[test]
    fn disk_invalid_numeric_request_creates_no_runtime_lease_or_operation() {
        let f = Fixture::new();
        let service = f.open();
        // Do not call current_revision: even internal inspection takes a resource lease.
        let mut r = ContextRequest {
            schema_version: 1,
            request_id: "invalid-numbers".into(),
            expected_provider_id: "synthetic-provider".into(),
            expected_revision: "0".repeat(64),
            target: Target::Common,
            patch: ContextPatch::default(),
        };
        for patch in [
            ContextPatch::default(),
            ContextPatch {
                model_context_window: Some(0),
                model_auto_compact_token_limit: None,
            },
            ContextPatch {
                model_context_window: Some(u64::MAX),
                model_auto_compact_token_limit: None,
            },
            ContextPatch {
                model_context_window: Some(1),
                model_auto_compact_token_limit: Some(2),
            },
        ] {
            r.patch = patch;
            assert_eq!(service.execute(&r), Err(ServiceError::InvalidRequest));
        }
        assert!(!f.runtime.join(crate::owner::RESOURCE_LEASE_FILE).exists());
        f.auth_unchanged();
    }

    #[test]
    fn disk_replace_rejection_and_failure_marker_commit_veto_remain_uncertain() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        for veto_marker in [false, true] {
            let f = Fixture::new();
            let service = f.open();
            let r = request(&service, "replace-rejection", Target::Common);
            if veto_marker {
                let count = Arc::new(AtomicUsize::new(0));
                service
                    .db
                    .with_conn(|c| {
                        c.commit_hook(Some(move || count.fetch_add(1, Ordering::SeqCst) + 1 == 3))
                            .map_err(|_| "synthetic hook installation".to_string())?;
                        Ok(())
                    })
                    .unwrap();
            }
            let result = service.execute_at(&r, Checkpoint::ReplaceDrift);
            if veto_marker {
                assert_eq!(result, Err(ServiceError::PersistenceUncertain));
            } else {
                let status = result.unwrap();
                assert_eq!(status.phase, Phase::ProjectionUncertain);
                assert_eq!(status.failure, Some(FailureCode::ProjectionOutcomeUnknown));
                assert_eq!(status.exit_code(), 3);
            }
            assert_eq!(f.live(), b"# synthetic external drift\n");
            let inode = fs::metadata(f.runtime.join("config.toml")).unwrap().ino();
            drop(service);
            let reopened = f.open();
            let status = reopened.execute(&r).unwrap();
            assert!(status.replay);
            assert_eq!(status.phase, Phase::ProjectionUncertain);
            assert_eq!(status.exit_code(), 3);
            assert_eq!(f.live(), b"# synthetic external drift\n");
            assert_eq!(
                fs::metadata(f.runtime.join("config.toml")).unwrap().ino(),
                inode
            );
            f.auth_unchanged();
        }
    }
    #[test]
    fn disk_replay_precedes_changed_provider_and_missing_runtime_inspection() {
        let f = Fixture::new();
        let service = f.open();
        let r = request(&service, "lookup-first", Target::Common);
        assert_eq!(service.execute(&r).unwrap().phase, Phase::Verified);
        service
            .db
            .with_conn(|c| {
                let mut provider: serde_json::Value =
                    db_get(c, DbTable::CodexProvider, "synthetic-provider")?.unwrap();
                provider["is_applied"] = json!(false);
                db_put(c, DbTable::CodexProvider, "synthetic-provider", &provider)
            })
            .unwrap();
        let status = service.execute(&r).unwrap();
        assert!(status.replay);
        assert_eq!(status.phase, Phase::Verified);
        let moved = f._root.path().join("moved-runtime");
        fs::rename(&f.runtime, &moved).unwrap();
        drop(service);
        let reopened = f.open();
        let status = reopened.execute(&r).unwrap();
        assert!(status.replay);
        assert_eq!(status.phase, Phase::Verified);
        assert!(!f.runtime.exists());
        assert_eq!(
            fs::read(moved.join("auth.json")).unwrap(),
            b"{\"synthetic\":\"not-a-credential\"}\n"
        );
        assert_eq!(
            fs::metadata(moved.join("auth.json")).unwrap().ino(),
            f.auth_inode
        );
    }
}
