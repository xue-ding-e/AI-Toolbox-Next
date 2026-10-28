//! Synthetic Linux gateway admission controls. No GUI, network, or real home.
use super::*;
use crate::management_admission::{test_session, GuiProfileSession};
use ai_toolbox_management::owner::{OrdinaryRuntimeLeaseGroup, ProfileLeaseStartup};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::sync::Arc;

const DENIED: &str = "This operation is not supported while managed Codex writes are enabled";
const CONFIG: &str = "model_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"http://127.0.0.1:37123/openai/v1\"\nwire_api = \"responses\"\nsupports_websockets = false\n";

struct Fixture {
    _directory: tempfile::TempDir,
    profile: PathBuf,
    runtime: PathBuf,
    paths: ProxyGatewayPaths,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::Builder::new().prefix("gateway-admission-")
            .tempdir_in(std::env::temp_dir()).unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let profile = directory.path().join("profile");
        let runtime = directory.path().join("runtime");
        for path in [&profile, &runtime] {
            fs::DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        fs::write(runtime.join("config.toml"), CONFIG).unwrap();
        fs::write(runtime.join("auth.json"), "{\"OPENAI_API_KEY\":\"synthetic\"}").unwrap();
        let paths = ProxyGatewayPaths::new(&profile);
        Self { _directory: directory, profile, runtime, paths }
    }

    fn session(&self, managed: bool) -> Arc<GuiProfileSession> {
        let startup = ProfileLeaseStartup::acquire(&self.profile).unwrap();
        if managed {
            GuiProfileSession::managed(startup.into_managed().unwrap())
        } else {
            GuiProfileSession::ordinary(startup.into_ordinary())
        }
    }

    fn targets(&self) -> CliProxyTargets {
        CliProxyTargets {
            runtime_root: self.runtime.clone(),
            is_wsl_direct: false,
            files: vec![
                CliProxyTarget { kind: CODEX_CONFIG_KIND, path: self.runtime.join("config.toml"),
                    managed_fields: codex_config_managed_fields_for_provider("custom") },
                CliProxyTarget { kind: CODEX_AUTH_KIND, path: self.runtime.join("auth.json"),
                    managed_fields: static_managed_fields(&CODEX_AUTH_MANAGED_FIELDS) },
            ],
        }
    }

    fn seed_manifest(&self, mode: GatewayProxyMode) -> CliProxyManifest {
        let mut manifest = CliProxyManifest::new(
            GatewayCliKey::Codex, "http://127.0.0.1:37123".into(), "synthetic".into(),
            mode, "provider".into(),
        );
        manifest.files = self.targets().files.iter().map(|target| CliProxyManifestFile {
            kind: target.kind.to_string(), path: path_to_string(&target.path), existed: false,
            backup_rel_path: format!("{}.bak", target.kind), backup_sha256: None,
            backup_size: None, managed_fields: target.managed_fields.clone(),
        }).collect();
        let path = self.paths.manifest_path(GatewayCliKey::Codex);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        manifest
    }
}

fn assert_denied<T>(result: Result<T, String>) {
    assert!(matches!(result, Err(ref error) if error == DENIED));
}

#[tokio::test(flavor = "current_thread")]
async fn managed_gateway_transactions_reject_before_validation_or_side_effects() {
    let fixture = Fixture::new();
    let session = fixture.session(true);
    let _scope = test_session(&session);
    let db = SqliteDbState::in_memory_for_test().unwrap();
    let status = ProxyGatewayStatus::stopped(&ProxyGatewaySettings::default(), None);
    // No running gateway/provider/manifest: the admission error must win even
    // over existing validation and no-op restore paths, for every CLI.
    for cli in GatewayCliKey::supported_mvp() {
        assert_denied(engage_single_cli(&db, &fixture.paths, cli, &status, "absent".into()).await);
        assert_denied(engage_failover_cli(&db, &fixture.paths, cli, &status).await);
        assert_denied(disengage_failover_cli(&db, &fixture.paths, cli, &status).await);
        assert_denied(restore_cli_direct(&db, &fixture.paths, cli, &status).await);
    }
    assert_denied(engage_aggregate_cli(
        &db, &fixture.paths, GatewayCliKey::Codex, &status, vec![], ".".into(),
        BTreeMap::new(), Default::default(), false, vec![], Default::default(),
    ).await);
    assert!(!fixture.paths.root().exists());
    assert_eq!(fs::read_to_string(fixture.runtime.join("config.toml")).unwrap(), CONFIG);
    assert_eq!(fs::read_to_string(fixture.runtime.join("auth.json")).unwrap(),
        "{\"OPENAI_API_KEY\":\"synthetic\"}");
}

#[tokio::test(flavor = "current_thread")]
async fn managed_provider_switch_rejects_before_application_state_lookup() {
    let fixture = Fixture::new();
    let session = fixture.session(true);
    let scope = test_session(&session);
    // Deliberately no registered DB or gateway state: reaching it would panic.
    let app = tauri::test::mock_app();
    for cli in GatewayCliKey::supported_mvp() {
        assert_denied(super::super::provider_switch::apply_or_switch_provider(
            app.handle(), cli, "absent", false,
        ).await);
    }
    drop(session);
    let result = super::super::provider_switch::apply_or_switch_provider(
        app.handle(), GatewayCliKey::Codex, "absent", false,
    ).await;
    assert!(matches!(result, Err(ref error) if error == "The GUI profile owner is unavailable"));
    drop(scope);
}

#[test]
fn managed_low_level_writers_reject_without_creating_backups_or_deleting_targets() {
    let fixture = Fixture::new();
    let manifest = fixture.seed_manifest(GatewayProxyMode::Single);
    let original_manifest = fs::read(fixture.paths.manifest_path(GatewayCliKey::Codex)).unwrap();
    let session = fixture.session(true);
    let _scope = test_session(&session);
    let config = fixture.runtime.join("config.toml");
    let auth = fixture.runtime.join("auth.json");
    assert_denied(prepare_manifest(&fixture.paths, GatewayCliKey::Codex, "http://localhost",
        &fixture.targets(), GatewayProxyMode::Single, "provider"));
    // existed=false would delete both files if this reached the restore body.
    assert_denied(restore_gateway_config(GatewayCliKey::Codex, &fixture.paths,
        &fixture.targets(), &manifest));
    assert_denied(patch_codex_config(&config, "http://localhost", false, GatewayProxyMode::Single, true));
    assert_denied(patch_codex_auth(&auth, false, None));
    assert_denied(restore_codex_config(&config, None, &DEFAULT_AGGREGATE_SUBAGENT_DEFAULTS));
    assert_denied(restore_codex_auth(&auth, None));
    assert_denied(apply_codex_aggregate_catalog_leave(&fixture.runtime, None));
    assert_denied(write_manifest(&fixture.paths, GatewayCliKey::Codex, &manifest));
    assert_eq!(fs::read_to_string(config).unwrap(), CONFIG);
    assert_eq!(fs::read_to_string(auth).unwrap(), "{\"OPENAI_API_KEY\":\"synthetic\"}");
    assert_eq!(fs::read(fixture.paths.manifest_path(GatewayCliKey::Codex)).unwrap(), original_manifest);
    assert!(!fixture.paths.backup_dir(GatewayCliKey::Codex).exists());
}

#[test]
fn managed_websocket_projection_preflights_before_settings_save_but_preserves_other_settings() {
    let fixture = Fixture::new();
    fixture.seed_manifest(GatewayProxyMode::Single);
    let db = SqliteDbState::in_memory_for_test().unwrap();
    let previous = settings::save_settings(&db, ProxyGatewaySettings::default()).unwrap();
    let session = fixture.session(true);
    let _scope = test_session(&session);
    let save = super::super::commands::save_settings_with_codex_projection;
    let mut changed = previous.clone();
    changed.codex_websocket_enabled = !previous.codex_websocket_enabled;
    changed.metrics_enabled = !previous.metrics_enabled;
    assert_denied(save(&db, &fixture.paths, changed));
    assert_eq!(serde_json::to_value(settings::load_settings_from_sqlite_state(&db).unwrap()).unwrap(),
        serde_json::to_value(&previous).unwrap());
    assert_eq!(fs::read_to_string(fixture.runtime.join("config.toml")).unwrap(), CONFIG);

    let mut unrelated = previous.clone();
    unrelated.metrics_enabled = !previous.metrics_enabled;
    let (saved, projection) = save(&db, &fixture.paths, unrelated).unwrap();
    assert!(projection.is_none());
    assert_ne!(saved.metrics_enabled, previous.metrics_enabled);
    // Aggregate never follows this gate; a toggle is a settings-only update.
    fixture.seed_manifest(GatewayProxyMode::Aggregate);
    let mut aggregate = saved;
    aggregate.codex_websocket_enabled = !previous.codex_websocket_enabled;
    assert!(save(&db, &fixture.paths, aggregate).unwrap().1.is_none());
    assert_eq!(fs::read_to_string(fixture.runtime.join("config.toml")).unwrap(), CONFIG);
}

#[test]
fn ordinary_projection_worker_retains_profile_and_runtime_after_caller_drops() {
    let fixture = Fixture::new();
    fixture.seed_manifest(GatewayProxyMode::Single);
    let session = fixture.session(false);
    let scope = test_session(&session);
    // The actual file helpers must reuse, not try to re-flock, this lease.
    let outer = crate::management_admission::admit_legacy_runtime(&fixture.runtime).unwrap();
    patch_codex_config(&fixture.runtime.join("config.toml"), "http://localhost", false,
        GatewayProxyMode::Single, false).unwrap();
    let projection = prepare_codex_websocket_projection(&fixture.paths, true).unwrap().unwrap();
    drop((outer, scope, session));

    let contender = Fixture::new();
    let profile = ProfileLeaseStartup::acquire(&contender.profile).unwrap().into_ordinary();
    assert!(matches!(ProfileLeaseStartup::acquire(&fixture.profile),
        Err(ai_toolbox_management::Error::OwnerBusy)));
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire(&profile, vec![fixture.runtime.clone()]),
        Err(ai_toolbox_management::Error::ResourceBusy)));
    // No thread-local test session on this worker. Its owned projection is the
    // lifetime authority even if the async/UI caller has already gone away.
    assert!(std::thread::spawn(move || projection.apply(true)).join().unwrap().unwrap());
    let document = parse_toml_file(&fixture.runtime.join("config.toml")).unwrap();
    assert_eq!(document["model_providers"]["custom"]["supports_websockets"].as_bool(), Some(true));
    drop(ProfileLeaseStartup::acquire(&fixture.profile).unwrap());
    drop(OrdinaryRuntimeLeaseGroup::acquire(&profile, vec![fixture.runtime.clone()]).unwrap());
}

#[test]
fn non_codex_target_alias_cannot_write_managed_codex_files() {
    let fixture = Fixture::new();
    let owner = ProfileLeaseStartup::acquire(&fixture.profile).unwrap().into_managed().unwrap();
    let runtime = ai_toolbox_management::owner::RuntimeResourceIdentity::existing(&fixture.runtime).unwrap();
    let session = GuiProfileSession::managed_with_runtime(owner, runtime).unwrap();
    let _scope = test_session(&session);
    let alias = fixture.profile.parent().unwrap().join("runtime-alias");
    std::os::unix::fs::symlink(&fixture.runtime, &alias).unwrap();
    let targets = CliProxyTargets {
        runtime_root: alias.clone(), is_wsl_direct: false,
        files: vec![CliProxyTarget { kind: KIMI_CONFIG_KIND, path: alias.join("config.toml"),
            managed_fields: vec![] }],
    };
    assert_denied(admit_gateway_target_write(GatewayCliKey::Kimi, &targets, None));
    assert_eq!(fs::read_to_string(fixture.runtime.join("config.toml")).unwrap(), CONFIG);
}
