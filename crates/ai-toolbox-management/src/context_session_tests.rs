//! Real Linux same-user IPC and shared file-DB fixtures. No installed CLI,
//! desktop admission, cross-user proof, or power-loss test is claimed here.

use super::*;
use ai_toolbox_storage::{
    codex_context_journal,
    helpers::{db_get, db_put},
    schema::DbTable,
    SQLITE_DATABASE_FILE,
};
use crate::{
    owner::{ProfileIdentity, ProfileOwner, RuntimeDirectory, RESOURCE_LEASE_FILE},
    transport::{
        endpoint_basename, ContextClientError, ContextRequestStatusRequest, ContextResult,
        ContextSetRequest, OwnerCapability, OwnerClient, OwnerServer,
    },
};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::PathBuf,
    sync::{atomic::{AtomicUsize, Ordering}, Arc},
    time::Duration,
};

struct Fixture {
    profile: PathBuf,
    runtime: PathBuf,
    ipc: PathBuf,
    _root: tempfile::TempDir,
}

struct Attached {
    db: SqliteDbState,
    session: Arc<ContextSession>,
    owner: OwnerPermit,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new().prefix("cs-")
            .permissions(fs::Permissions::from_mode(0o700)).tempdir().unwrap();
        let profile = root.path().join("p");
        let runtime = root.path().join("c");
        let ipc = root.path().join("i");
        for path in [&profile, &runtime, &ipc] {
            fs::DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        fs::write(runtime.join("config.toml"), concat!(
            "# do-not-reflect-config\nmodel_context_window = 1000000\n",
            "model_auto_compact_token_limit = 900000\n",
            "model_provider = \"synthetic\"\n[model_providers.synthetic]\n",
            "base_url = \"https://invalid.example\"\n",
        )).unwrap();
        fs::write(runtime.join("auth.json"), b"synthetic-auth-marker\n").unwrap();
        for leaf in ["config.toml", "auth.json"] {
            fs::set_permissions(runtime.join(leaf), fs::Permissions::from_mode(0o600)).unwrap();
        }
        Self { profile, runtime, ipc, _root: root }
    }

    fn attach(&self, seed: bool) -> Attached {
        // This fixture follows the real caller contract: owner before DB open.
        let owner = ProfileOwner::acquire(ProfileIdentity::existing(&self.profile).unwrap()).unwrap();
        let db = SqliteDbState::open(self.profile.join(SQLITE_DATABASE_FILE)).unwrap();
        fs::set_permissions(db.db_path(), fs::Permissions::from_mode(0o600)).unwrap();
        if seed {
            db.with_conn(|connection| {
                db_put(connection, DbTable::CodexCommonConfig, "common", &json!({
                    "config": "# do-not-reflect-config\nmodel_context_window = 1000000\n",
                    "root_dir": self.runtime.to_str().unwrap(),
                    "unknown": {"preserved": true},
                }))?;
                db_put(connection, DbTable::CodexProvider, "synthetic-provider", &json!({
                    "is_applied": true, "is_disabled": false,
                    "unknown_outer": {"preserved": true},
                    "settings_config": json!({
                        "config": "model_auto_compact_token_limit = 900000\n",
                        "auth": {"synthetic": "synthetic-auth-marker"},
                    }).to_string(),
                }))
            }).unwrap();
        }
        let session = Arc::new(ContextSession::attach(&owner, &db).unwrap());
        Attached { db, session, owner }
    }

    fn server(&self, attached: &Attached) -> OwnerServer {
        OwnerServer::bind(attached.owner.clone(), RuntimeDirectory::existing(&self.ipc).unwrap()).unwrap()
    }

    fn start_client(&self, operation: OwnerOperation) -> tokio::task::JoinHandle<ContextResult<ContextResponse>> {
        let profile = ProfileIdentity::existing(&self.profile).unwrap();
        let ipc = RuntimeDirectory::existing(&self.ipc).unwrap();
        tokio::spawn(async move {
            let client = OwnerClient::connect(&profile, &ipc, 7).await?;
            match operation {
                OwnerOperation::ContextInspect => client.context_inspect().await.map(ContextResponse::Inspect),
                OwnerOperation::ContextSet(request) => client.context_set(request).await.map(ContextResponse::Outcome),
                OwnerOperation::ContextRequestStatus(request) => client.context_request_status(request.request_id)
                    .await.map(ContextResponse::Outcome),
                OwnerOperation::OwnerStatus | OwnerOperation::OwnerCapabilities => {
                    panic!("use the dedicated owner responder");
                }
            }
        })
    }

    async fn request(
        &self,
        server: &OwnerServer,
        session: &Arc<ContextSession>,
        operation: OwnerOperation,
    ) -> ContextResult<ContextResponse> {
        let client = self.start_client(operation);
        let pending = server.accept().await.unwrap();
        let session = session.clone();
        let (pending, response) = tokio::task::spawn_blocking(move || {
            let response = session.execute_pending(&pending);
            (pending, response)
        }).await.unwrap();
        let response = response.unwrap();
        self.assert_sanitized(&response);
        pending.respond_context(response).await.unwrap();
        client.await.unwrap()
    }

    async fn inspect(&self, server: &OwnerServer, session: &Arc<ContextSession>) -> ContextInspection {
        match self.request(server, session, OwnerOperation::ContextInspect).await.unwrap() {
            ContextResponse::Inspect(value) => value,
            _ => panic!("expected typed inspection"),
        }
    }

    fn assert_sanitized(&self, response: &ContextResponse) {
        let wire = serde_json::to_string(response).unwrap();
        for forbidden in ["do-not-reflect-config", "synthetic-auth-marker", "settings_config",
            "request_fingerprint", "planned_live_digest", self._root.path().to_str().unwrap()]
        {
            assert!(!wire.contains(forbidden));
        }
        if let ContextResponse::Inspect(_) = response {
            let value = serde_json::to_value(response).unwrap();
            assert_eq!(value["inspect"].as_object().unwrap().len(), 4);
            for field in ["model_context_window", "model_auto_compact_token_limit"] {
                assert_eq!(value["inspect"][field].as_object().unwrap().len(), 2);
            }
        }
    }

    fn live(&self) -> Vec<u8> { fs::read(self.runtime.join("config.toml")).unwrap() }
}

fn run(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread().enable_io().enable_time()
        .max_blocking_threads(2).build().unwrap().block_on(async {
            tokio::time::timeout(Duration::from_secs(20), future).await.unwrap();
        });
}

fn set_request(inspection: &ContextInspection, id: &str) -> ContextSetRequest {
    ContextSetRequest {
        schema_version: 1,
        request_id: id.into(),
        expected_provider_id: inspection.provider_id.clone(),
        expected_revision: inspection.revision.clone(),
        target: ContextTarget::Common,
        patch: crate::transport::ContextPatch {
            model_context_window: Some(2_000_000),
            model_auto_compact_token_limit: None,
        },
    }
}

fn status(id: &str) -> OwnerOperation {
    OwnerOperation::ContextRequestStatus(ContextRequestStatusRequest { request_id: id.into() })
}

fn expect_outcome(result: ContextResult<ContextResponse>) -> ContextOutcome {
    match result.unwrap() {
        ContextResponse::Outcome(value) => value,
        _ => panic!("expected typed outcome"),
    }
}

fn expect_error(result: ContextResult<ContextResponse>, code: ContextErrorCode) {
    match result {
        Err(ContextClientError::Rejected(error)) => {
            assert_eq!(error.code, code);
            assert!(error.partial.is_none());
        }
        other => panic!("expected static rejection, received {other:?}"),
    }
}

fn changes(db: &SqliteDbState) -> u64 { db.with_conn(|connection| Ok(connection.total_changes())).unwrap() }

fn record(db: &SqliteDbState, table: DbTable, id: &str) -> serde_json::Value {
    db.with_conn(|connection| db_get(connection, table, id)).unwrap().unwrap()
}

#[test]
fn service_error_codes_and_checked_numeric_mapping_are_exact() {
    for (error, code) in [
        (ServiceError::Ownership, ContextErrorCode::Ownership),
        (ServiceError::Storage, ContextErrorCode::Storage),
        (ServiceError::InvalidMaster, ContextErrorCode::InvalidMaster),
        (ServiceError::InvalidRequest, ContextErrorCode::InvalidRequest),
        (ServiceError::ProviderChanged, ContextErrorCode::ProviderChanged),
        (ServiceError::RevisionConflict, ContextErrorCode::RevisionConflict),
        (ServiceError::RuntimeUnsupported, ContextErrorCode::RuntimeUnsupported),
        (ServiceError::FileUnavailable, ContextErrorCode::FileUnavailable),
        (ServiceError::JournalConflict, ContextErrorCode::JournalConflict),
        (ServiceError::JournalInvalid, ContextErrorCode::JournalInvalid),
        (ServiceError::PersistenceUncertain, ContextErrorCode::PersistenceUncertain),
        (ServiceError::PhasePersistence { last_committed: Phase::ProjectedUnverified }, ContextErrorCode::PhasePersistence),
        (ServiceError::Interrupted, ContextErrorCode::Interrupted),
    ] {
        assert_eq!(service_error(error), ContextError { code, partial: None });
    }
    for invalid in [-1, 0] {
        assert_eq!(positive_value(Some(invalid), ContextErrorCode::JournalInvalid),
            Err(context_error(ContextErrorCode::JournalInvalid)));
    }
    assert_eq!(positive_value(None, ContextErrorCode::InvalidMaster), Ok(None));
    assert_eq!(positive_value(Some(i64::MAX), ContextErrorCode::InvalidMaster), Ok(Some(9_223_372_036_854_775_807)));
    for (phase, expected) in [
        (Phase::SavedOnly, ContextPhase::SavedOnly),
        (Phase::ProjectionStarted, ContextPhase::ProjectionStarted),
        (Phase::ProjectedUnverified, ContextPhase::ProjectedUnverified),
        (Phase::Verified, ContextPhase::Verified),
        (Phase::ProjectionUncertain, ContextPhase::ProjectionUncertain),
    ] {
        let mapped = outcome("request".into(), "historical-provider".into(), ContextStatus {
            phase, failure: None, replay: false, context: Some(1), auto_compact: None,
        }).unwrap();
        assert_eq!(mapped.phase, expected);
        assert_eq!(mapped.provider_id, "historical-provider");
        assert!(!mapped.replay);
    }
    for (failure, phase, expected) in [
        (FailureCode::ProjectionPreflightFailed, Phase::SavedOnly, ContextFailure::ProjectionPreflightFailed),
        (FailureCode::ProjectionOutcomeUnknown, Phase::ProjectionUncertain, ContextFailure::ProjectionOutcomeUnknown),
        (FailureCode::ReadbackFailed, Phase::ProjectedUnverified, ContextFailure::ReadbackFailed),
        (FailureCode::ReadbackMismatch, Phase::ProjectedUnverified, ContextFailure::ReadbackMismatch),
    ] {
        let mapped = outcome("request".into(), "provider".into(), ContextStatus {
            phase, failure: Some(failure), replay: true,
            context: Some(1), auto_compact: None,
        }).unwrap();
        assert_eq!(mapped.failure, Some(expected));
        assert!(mapped.replay);
    }
    for (source, expected, value) in [
        (Source::Common, ContextSource::Common, Some(1)),
        (Source::Provider, ContextSource::Provider, Some(1)),
        (Source::Unspecified, ContextSource::Unspecified, None),
    ] {
        let mapped = inspection(codex_service::ContextInspection {
            provider_id: "provider".into(), revision: "a".repeat(64),
            model_context_window: codex_service::InspectedOverride { effective: value, source },
            model_auto_compact_token_limit: codex_service::InspectedOverride { effective: None, source: Source::Unspecified },
        }).unwrap();
        assert_eq!(mapped.model_context_window.source, expected);
        assert_eq!(mapped.model_context_window.effective, value.map(|value| u64::try_from(value).unwrap()));
    }
}

#[test]
fn owner_status_and_capability_requests_keep_their_dedicated_host_responders() {
    run(async {
        let fixture = Fixture::new();
        let attached = fixture.attach(true);
        let server = fixture.server(&attached);
        for capabilities in [false, true] {
            let profile = ProfileIdentity::existing(&fixture.profile).unwrap();
            let ipc = RuntimeDirectory::existing(&fixture.ipc).unwrap();
            let client = tokio::spawn(async move {
                let client = OwnerClient::connect(&profile, &ipc, 9).await?;
                if capabilities {
                    assert_eq!(client.owner_capabilities().await?, OwnerCapability::StatusOnly);
                } else {
                    assert!(client.owner_status().await?.profile_lease_held);
                }
                Ok::<(), crate::Error>(())
            });
            let pending = server.accept().await.unwrap();
            let before = changes(&attached.db);
            assert!(matches!(attached.session.execute_pending(&pending), Err(crate::Error::InvalidMessage)));
            assert_eq!(changes(&attached.db), before);
            if capabilities {
                pending.respond_capabilities(OwnerCapability::StatusOnly).await.unwrap();
            } else {
                pending.respond_status().await.unwrap();
            }
            client.await.unwrap().unwrap();
        }
        assert!(!fixture.runtime.join(RESOURCE_LEASE_FILE).exists());
        drop(server);
        drop(attached);
    });
}

#[test]
fn ipc_inspect_uses_original_uncommitted_connection_without_runtime_lease() {
    run(async {
        let fixture = Fixture::new();
        let attached = fixture.attach(true);
        let server = fixture.server(&attached);
        let live = fixture.live();
        let inode = fs::metadata(fixture.runtime.join("config.toml")).unwrap().ino();
        let mut common = record(&attached.db, DbTable::CodexCommonConfig, "common");
        common["config"] = json!("model_context_window = 1500000\n");
        attached.db.with_conn(|connection| {
            connection.execute_batch("BEGIN").map_err(|error| error.to_string())?;
            db_put(connection, DbTable::CodexCommonConfig, "common", &common)
        }).unwrap();
        let before = changes(&attached.db);
        fs::set_permissions(fixture.runtime.join("config.toml"), fs::Permissions::from_mode(0o400)).unwrap();
        fs::set_permissions(&fixture.runtime, fs::Permissions::from_mode(0o500)).unwrap();
        let inspected = fixture.inspect(&server, &attached.session).await;
        assert_eq!(inspected.model_context_window, ContextOverride { effective: Some(1_500_000), source: ContextSource::Common });
        assert_eq!(inspected.model_auto_compact_token_limit, ContextOverride { effective: Some(900_000), source: ContextSource::Provider });
        assert_eq!(changes(&attached.db), before);
        assert_eq!(fixture.live(), live);
        assert_eq!(fs::metadata(fixture.runtime.join("config.toml")).unwrap().ino(), inode);
        assert!(!fixture.runtime.join(RESOURCE_LEASE_FILE).exists());
        attached.db.with_conn(|connection| {
            assert!(!connection.is_autocommit());
            connection.execute_batch("ROLLBACK").map_err(|error| error.to_string())
        }).unwrap();
        assert_eq!(fixture.inspect(&server, &attached.session).await.model_context_window.effective, Some(1_000_000));
        fs::set_permissions(&fixture.runtime, fs::Permissions::from_mode(0o700)).unwrap();
        drop(server);
        drop(attached);
    });
}

#[test]
fn ipc_set_replay_conflict_and_reopen_use_original_connection_and_real_effects() {
    run(async {
        let fixture = Fixture::new();
        let attached = fixture.attach(true);
        let server = fixture.server(&attached);
        let old_epoch = attached.owner.epoch();
        let inspected = fixture.inspect(&server, &attached.session).await;
        let request = set_request(&inspected, "set-reopen");
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = calls.clone();
        attached.db.with_conn(|connection| {
            connection.commit_hook(Some(move || { hook_calls.fetch_add(1, Ordering::SeqCst); false }));
            Ok(())
        }).unwrap();
        let auth = fs::read(fixture.runtime.join("auth.json")).unwrap();
        let auth_inode = fs::metadata(fixture.runtime.join("auth.json")).unwrap().ino();
        let result = expect_outcome(fixture.request(&server, &attached.session, OwnerOperation::ContextSet(request.clone())).await);
        assert_eq!(result.phase, ContextPhase::Verified);
        assert!(!result.replay);
        assert_eq!(result.model_context_window, Some(2_000_000));
        assert_eq!(result.model_auto_compact_token_limit, Some(900_000));
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        let common = record(&attached.db, DbTable::CodexCommonConfig, "common");
        assert!(common["config"].as_str().unwrap().contains("model_context_window = 2000000"));
        assert_eq!(common["unknown"]["preserved"], true);
        assert_eq!(record(&attached.db, DbTable::CodexProvider, "synthetic-provider")["unknown_outer"]["preserved"], true);
        assert!(String::from_utf8(fixture.live()).unwrap().contains("model_context_window = 2000000"));
        let live = fixture.live();
        let inode = fs::metadata(fixture.runtime.join("config.toml")).unwrap().ino();
        let before = changes(&attached.db);
        let replay = expect_outcome(fixture.request(&server, &attached.session, OwnerOperation::ContextSet(request.clone())).await);
        assert_eq!(replay, ContextOutcome { replay: true, ..result.clone() });
        let mut conflict = request.clone();
        conflict.patch.model_context_window = Some(3_000_000);
        expect_error(fixture.request(&server, &attached.session, OwnerOperation::ContextSet(conflict)).await, ContextErrorCode::JournalConflict);
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert_eq!(changes(&attached.db), before);
        assert_eq!(fixture.live(), live);
        assert_eq!(fs::metadata(fixture.runtime.join("config.toml")).unwrap().ino(), inode);
        assert_eq!(fs::read(fixture.runtime.join("auth.json")).unwrap(), auth);
        assert_eq!(fs::metadata(fixture.runtime.join("auth.json")).unwrap().ino(), auth_inode);
        let mut stale = request.clone();
        stale.request_id = "new-stale".into();
        expect_error(fixture.request(&server, &attached.session, OwnerOperation::ContextSet(stale)).await, ContextErrorCode::RevisionConflict);
        drop(server);
        drop(attached);

        let reopened = fixture.attach(false);
        assert_ne!(reopened.owner.epoch(), old_epoch);
        let server = fixture.server(&reopened);
        let before = changes(&reopened.db);
        assert_eq!(expect_outcome(fixture.request(&server, &reopened.session, status(&request.request_id)).await), replay);
        assert_eq!(changes(&reopened.db), before);
        assert_eq!(fixture.live(), live);
        drop(server);
        drop(reopened);
    });
}

#[test]
fn ipc_partial_and_historical_status_do_not_resume_or_require_current_provider_runtime_or_gate() {
    run(async {
        let fixture = Fixture::new();
        let attached = fixture.attach(true);
        let server = fixture.server(&attached);
        fs::set_permissions(fixture.runtime.join("config.toml"), fs::Permissions::from_mode(0o400)).unwrap();
        let request = set_request(&fixture.inspect(&server, &attached.session).await, "saved-only");
        let live = fixture.live();
        let result = expect_outcome(fixture.request(&server, &attached.session, OwnerOperation::ContextSet(request.clone())).await);
        assert_eq!(result.phase, ContextPhase::SavedOnly);
        assert_eq!(result.failure, Some(ContextFailure::ProjectionPreflightFailed));
        assert_ne!(result.phase, ContextPhase::Verified);
        assert_eq!(fixture.live(), live);
        let mut provider = record(&attached.db, DbTable::CodexProvider, "synthetic-provider");
        provider["is_applied"] = json!(false);
        attached.db.with_conn(|connection| db_put(connection, DbTable::CodexProvider, "synthetic-provider", &provider)).unwrap();
        fs::rename(&fixture.runtime, fixture._root.path().join("moved-runtime")).unwrap();
        let writer = attached.owner.try_writer().unwrap();
        let before = changes(&attached.db);
        let historical = expect_outcome(fixture.request(&server, &attached.session, status(&request.request_id)).await);
        assert_eq!(historical, ContextOutcome { replay: true, ..result });
        expect_error(fixture.request(&server, &attached.session, status("missing-request")).await, ContextErrorCode::RequestNotFound);
        assert_eq!(changes(&attached.db), before);
        drop(writer);
        drop(server);
        drop(attached);
        let reopened = fixture.attach(false);
        let server = fixture.server(&reopened);
        assert_eq!(expect_outcome(fixture.request(&server, &reopened.session, status(&request.request_id)).await), historical);
        assert_eq!(fs::read(fixture._root.path().join("moved-runtime/config.toml")).unwrap(), live);
        drop(server);
        drop(reopened);
    });
}

#[test]
fn ipc_real_commit_veto_returns_uncertain_error_without_inventing_zero_effects_or_partial() {
    run(async {
        for (veto, persisted_phase, file_changed) in [
            (1, None, false),
            (2, Some(ContextPhase::SavedOnly), false),
            (3, Some(ContextPhase::ProjectionUncertain), true),
            (4, Some(ContextPhase::ProjectedUnverified), true),
        ] {
            let fixture = Fixture::new();
            let attached = fixture.attach(true);
            let server = fixture.server(&attached);
            let request = set_request(&fixture.inspect(&server, &attached.session).await, "commit-veto");
            let live = fixture.live();
            let calls = Arc::new(AtomicUsize::new(0));
            let hook_calls = calls.clone();
            attached.db.with_conn(|connection| {
                connection.commit_hook(Some(move || hook_calls.fetch_add(1, Ordering::SeqCst) + 1 == veto));
                Ok(())
            }).unwrap();
            expect_error(fixture.request(&server, &attached.session, OwnerOperation::ContextSet(request.clone())).await,
                ContextErrorCode::PersistenceUncertain);
            assert_eq!(calls.load(Ordering::SeqCst), veto);
            assert_eq!(fixture.live() != live, file_changed);
            let before = changes(&attached.db);
            let queried = fixture.request(&server, &attached.session, status(&request.request_id)).await;
            match persisted_phase {
                None => expect_error(queried, ContextErrorCode::RequestNotFound),
                Some(phase) => assert_eq!(expect_outcome(queried).phase, phase),
            }
            assert_eq!(changes(&attached.db), before);
            assert_eq!(calls.load(Ordering::SeqCst), veto);
            drop(server);
            drop(attached);
            let reopened = fixture.attach(false);
            let server = fixture.server(&reopened);
            let queried = fixture.request(&server, &reopened.session, status(&request.request_id)).await;
            match persisted_phase {
                None => expect_error(queried, ContextErrorCode::RequestNotFound),
                Some(phase) => assert_eq!(expect_outcome(queried).phase, phase),
            }
            drop(server);
            drop(reopened);
        }
    });
}

#[test]
fn authenticated_pending_cannot_use_another_sessions_profile() {
    run(async {
        let fixture = Fixture::new();
        let other = Fixture::new();
        let attached = fixture.attach(true);
        let unrelated = other.attach(true);
        let server = fixture.server(&attached);
        let request = set_request(&fixture.inspect(&server, &attached.session).await, "foreign-session");
        let client = fixture.start_client(OwnerOperation::ContextSet(request));
        let pending = server.accept().await.unwrap();
        assert_eq!(pending.request().profile, attached.owner.profile().key());
        assert_eq!(pending.request().owner_epoch, attached.owner.epoch());
        let before = (changes(&attached.db), changes(&unrelated.db), fixture.live(), other.live());
        let session = unrelated.session.clone();
        let result = tokio::task::spawn_blocking(move || session.execute_pending(&pending)).await.unwrap();
        assert!(matches!(result, Err(crate::Error::StaleOwner)));
        assert!(matches!(client.await.unwrap(), Err(ContextClientError::OutcomeUnknown)));
        assert_eq!((changes(&attached.db), changes(&unrelated.db), fixture.live(), other.live()), before);
        drop(server);
        drop(attached);
        drop(unrelated);
    });
}

#[test]
fn queued_pending_rechecks_endpoint_identity_inside_blocking_execution() {
    run(async {
        for replace in ["socket", "runtime", "profile"] {
            let fixture = Fixture::new();
            let attached = fixture.attach(true);
            let server = fixture.server(&attached);
            let client = fixture.start_client(OwnerOperation::ContextInspect);
            let pending = server.accept().await.unwrap();
            let before = changes(&attached.db);
            let held = fixture._root.path().join("held-original");
            let target = match replace {
                "socket" => fixture.ipc.join(endpoint_basename(attached.owner.profile().key())),
                "runtime" => fixture.ipc.clone(),
                "profile" => fixture.profile.clone(),
                _ => unreachable!(),
            };
            fs::rename(&target, &held).unwrap();
            if replace != "socket" {
                fs::DirBuilder::new().mode(0o700).create(&target).unwrap();
            }
            let session = attached.session.clone();
            let (pending, result) = tokio::task::spawn_blocking(move || {
                let result = session.execute_pending(&pending);
                (pending, result)
            }).await.unwrap();
            assert!(matches!(result, Err(crate::Error::IdentityChanged | crate::Error::OwnerUnavailable)));
            assert_eq!(changes(&attached.db), before);
            if replace != "socket" { fs::remove_dir(&target).unwrap(); }
            fs::rename(&held, &target).unwrap();
            drop(pending);
            assert!(client.await.unwrap().is_err());
            drop(server);
            drop(attached);
        }
    });
}

#[test]
fn attached_service_keeps_connection_identity_checks_and_single_writer_gate() {
    run(async {
        let fixture = Fixture::new();
        let attached = fixture.attach(true);
        let server = fixture.server(&attached);
        let request = set_request(&fixture.inspect(&server, &attached.session).await, "writer-busy");
        let writer = attached.owner.try_writer().unwrap();
        expect_error(fixture.request(&server, &attached.session, OwnerOperation::ContextSet(request)).await, ContextErrorCode::Ownership);
        expect_error(fixture.request(&server, &attached.session, status("writer-busy")).await, ContextErrorCode::RequestNotFound);
        drop(writer);
        let client = fixture.start_client(OwnerOperation::ContextInspect);
        let pending = server.accept().await.unwrap();
        let database = fixture.profile.join(SQLITE_DATABASE_FILE);
        let held = fixture.profile.join("held-database");
        let before = changes(&attached.db);
        fs::rename(&database, &held).unwrap();
        let session = attached.session.clone();
        let (pending, response) = tokio::task::spawn_blocking(move || {
            let response = session.execute_pending(&pending);
            (pending, response)
        }).await.unwrap();
        assert_eq!(response.unwrap(), ContextResponse::Error(context_error(ContextErrorCode::Storage)));
        assert_eq!(changes(&attached.db), before);
        fs::rename(&held, &database).unwrap();
        pending.respond_context(ContextResponse::Error(context_error(ContextErrorCode::Storage))).await.unwrap();
        expect_error(client.await.unwrap(), ContextErrorCode::Storage);
        drop(server);
        drop(attached);
    });
}

#[test]
fn cancelling_waiter_and_disconnecting_client_retain_session_until_actual_work_finishes() {
    run(async {
        let fixture = Fixture::new();
        let attached = fixture.attach(true);
        let server = fixture.server(&attached);
        let request = set_request(&fixture.inspect(&server, &attached.session).await, "cancel-waiter");
        let client = fixture.start_client(OwnerOperation::ContextSet(request.clone()));
        let pending = server.accept().await.unwrap();
        let weak = Arc::downgrade(&attached.session);
        let session = attached.session.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let blocking = tokio::task::spawn_blocking(move || {
            started_tx.send(()).unwrap();
            // Bounded even if the test's async assertion fails; runtime drop
            // must not hang forever waiting for a synthetic blocking worker.
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            let result = session.execute_pending(&pending);
            drop(pending);
            drop(session);
            let _ = done_tx.send(result);
        });
        let waiter = tokio::spawn(async move { blocking.await.unwrap() });
        started_rx.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        client.abort();
        assert!(client.await.unwrap_err().is_cancelled());
        drop(server);
        drop(attached);
        assert!(weak.upgrade().is_some());
        assert!(matches!(ProfileOwner::acquire(ProfileIdentity::existing(&fixture.profile).unwrap()), Err(crate::Error::OwnerBusy)));
        release_tx.send(()).unwrap();
        assert!(matches!(done_rx.await.unwrap().unwrap(), ContextResponse::Outcome(ContextOutcome { phase: ContextPhase::Verified, .. })));
        assert!(weak.upgrade().is_none());
        let reopened = fixture.attach(false);
        let server = fixture.server(&reopened);
        let result = expect_outcome(fixture.request(&server, &reopened.session, status(&request.request_id)).await);
        assert_eq!(result.phase, ContextPhase::Verified);
        assert!(result.replay);
        assert!(String::from_utf8(fixture.live()).unwrap().contains("model_context_window = 2000000"));
        drop(server);
        drop(reopened);
    });
}

#[test]
fn attach_failure_is_static_and_does_not_open_or_initialize_a_missing_database() {
    let fixture = Fixture::new();
    let owner = ProfileOwner::acquire(ProfileIdentity::existing(&fixture.profile).unwrap()).unwrap();
    let db = SqliteDbState::in_memory_for_test().unwrap();
    assert!(matches!(ContextSession::attach(&owner, &db), Err(ContextError { code: ContextErrorCode::Storage, partial: None })));
    assert!(!fixture.profile.join(SQLITE_DATABASE_FILE).exists());
    assert!(!fixture.runtime.join(RESOURCE_LEASE_FILE).exists());
    // Failed attach installed no owner retention on this unrelated connection.
    drop(owner);
    let owner = ProfileOwner::acquire(ProfileIdentity::existing(&fixture.profile).unwrap()).unwrap();
    let row = db.with_conn(|connection| codex_context_journal::read_status(connection, "missing")
        .map_err(|error| error.to_string())).unwrap();
    assert!(row.is_none());
    drop(db);
    drop(owner);
}
