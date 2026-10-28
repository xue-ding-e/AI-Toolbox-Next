// Closed-wire and synthetic transport tests only. No business dispatcher,
// database/file effect, installed binary, or native cross-user proof is claimed.
use super::*;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

fn set_request() -> ContextSetRequest {
    ContextSetRequest {
        schema_version: 1,
        request_id: "context-request_1".into(),
        expected_provider_id: "provider_1".into(),
        expected_revision: "a".repeat(64),
        target: ContextTarget::Provider,
        patch: ContextPatch {
            model_context_window: Some(128_000),
            model_auto_compact_token_limit: Some(96_000),
        },
    }
}

fn inspection() -> ContextInspection {
    ContextInspection {
        provider_id: "provider_1".into(),
        revision: "a".repeat(64),
        model_context_window: ContextOverride { effective: Some(128_000), source: ContextSource::Provider },
        model_auto_compact_token_limit: ContextOverride { effective: Some(96_000), source: ContextSource::Common },
    }
}

fn outcome() -> ContextOutcome {
    ContextOutcome {
        request_id: set_request().request_id,
        provider_id: set_request().expected_provider_id,
        phase: ContextPhase::Verified,
        failure: None,
        replay: false,
        model_context_window: Some(128_000),
        model_auto_compact_token_limit: Some(96_000),
    }
}

#[test]
fn legacy_status_wire_and_major_remain_exact() {
    assert_eq!(PROTOCOL_MAJOR, 1);
    assert_eq!(serde_json::to_string(&OwnerOperation::OwnerStatus).unwrap(), r#""owner_status""#);
    assert_eq!(serde_json::to_string(&OwnerStatus { profile_lease_held: true }).unwrap(), r#"{"profile_lease_held":true}"#);
    let original = r#"{"protocol_major":1,"request_id":7,"profile":{"device":1,"inode":2},"owner_epoch":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"status":{"profile_lease_held":true}}"#;
    let decoded: OwnerResponse = serde_json::from_str(original).unwrap();
    assert_eq!(serde_json::to_string(&decoded).unwrap(), original);
    let original_request = r#"{"protocol_major":1,"request_id":7,"profile":{"device":1,"inode":2},"owner_epoch":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"operation":"owner_status"}"#;
    let decoded: OwnerRequest = serde_json::from_str(original_request).unwrap();
    assert_eq!(serde_json::to_string(&decoded).unwrap(), original_request);
}

#[test]
fn context_wire_is_explicit_and_has_no_generic_payload() {
    assert_eq!(serde_json::to_string(&OwnerOperation::ContextInspect).unwrap(), r#""context_inspect""#);
    let request = set_request();
    let wire = serde_json::to_string(&OwnerOperation::ContextSet(request.clone())).unwrap();
    assert!(wire.starts_with(r#"{"context_set":{"schema_version":1,"request_id":"context-request_1""#));
    assert_eq!(serde_json::from_str::<OwnerOperation>(&wire).unwrap(), OwnerOperation::ContextSet(request));
    assert_eq!(serde_json::to_string(&OwnerOperation::ContextRequestStatus(ContextRequestStatusRequest { request_id: "r1".into() })).unwrap(), r#"{"context_request_status":{"request_id":"r1"}}"#);
    for wire in [r#""invoke""#, r#"{"context_inspect":{"rawValue":"secret"}}"#, r#"{"context_request_status":{"request_id":"r1","path":"/tmp/private"}}"#] {
        assert!(serde_json::from_str::<OwnerOperation>(wire).is_err());
    }
}

#[test]
fn context_dtos_reject_unknown_duplicate_and_wrong_type_fields() {
    let valid = serde_json::to_string(&set_request()).unwrap();
    for field in ["rawValue", "toml", "sql", "path", "auth", "extra"] {
        let wire = valid.replacen('{', &format!(r#"{{"{field}":"secret","#), 1);
        assert!(serde_json::from_str::<ContextSetRequest>(&wire).is_err());
        let wire = valid.replace(r#""patch":{"#, &format!(r#""patch":{{"{field}":"secret","#));
        assert!(serde_json::from_str::<ContextSetRequest>(&wire).is_err());
    }
    for extra in [r#""schema_version":1,"#, r#""request_id":"r2","#, r#""patch":{},"#] {
        let wire = valid.replacen('{', &format!("{{{extra}"), 1);
        assert!(serde_json::from_str::<ContextSetRequest>(&wire).is_err());
    }
    for value in ["-1", "1.0", "true", r#""1""#, "18446744073709551616"] {
        let wire = valid.replace("128000", value);
        assert!(serde_json::from_str::<ContextSetRequest>(&wire).is_err());
    }
    for wire in [
        r#"{"model_context_window":null,"model_context_window":1}"#,
        r#"{"model_context_window":1,"model_context_window":2}"#,
        r#"{"model_auto_compact_token_limit":null,"model_auto_compact_token_limit":1}"#,
    ] {
        assert!(serde_json::from_str::<ContextPatch>(wire).is_err());
    }
    assert!(serde_json::from_str::<ContextError>(r#"{"code":"storage","partial":null,"message":"raw error"}"#).is_err());
    assert!(serde_json::from_str::<ContextError>(r#"{"code":"storage","code":"invalid_request","partial":null}"#).is_err());
    assert!(serde_json::from_str::<ContextResponse>(r#"{"error":{"code":"unreviewed_error","partial":null}}"#).is_err());
}

#[test]
fn bounded_ids_revision_schema_and_numeric_patch_are_enforced() {
    let base = set_request();
    assert!(base.validate().is_ok());
    for value in ["".to_owned(), "a".repeat(129), "../a".into(), "a:b".into(), "你好".into(), "a b".into()] {
        let mut request = base.clone();
        request.request_id = value.clone();
        assert!(request.validate().is_err());
        request = base.clone();
        request.expected_provider_id = value;
        assert!(request.validate().is_err());
    }
    for revision in ["a".repeat(63), "a".repeat(65), "A".repeat(64), "g".repeat(64)] {
        let mut request = base.clone();
        request.expected_revision = revision;
        assert!(request.validate().is_err());
    }
    let mut request = base.clone();
    request.schema_version = 2;
    assert!(request.validate().is_err());
    for (context, compact) in [(None, None), (Some(0), None), (None, Some(0)), (Some(u64::MAX), None), (Some(1), Some(2))] {
        let mut request = base.clone();
        request.patch = ContextPatch { model_context_window: context, model_auto_compact_token_limit: compact };
        assert!(request.validate().is_err());
    }
    for (context, compact) in [(Some(1), None), (None, Some(1)), (Some(i64::MAX as u64), Some(i64::MAX as u64))] {
        let mut request = base.clone();
        request.patch = ContextPatch { model_context_window: context, model_auto_compact_token_limit: compact };
        assert!(request.validate().is_ok());
    }
    let mut request = base;
    request.request_id = "_".repeat(128);
    request.expected_provider_id = "-".repeat(128);
    request.expected_revision = "0123456789abcdef".repeat(4);
    assert!(request.validate().is_ok());
}

#[test]
fn response_kinds_identity_and_partial_states_are_bound() {
    let set = OwnerOperation::ContextSet(set_request());
    let inspect = ContextResponse::Inspect(inspection());
    assert!(inspect.validate_for(&OwnerOperation::ContextInspect).is_ok());
    assert!(inspect.validate_for(&set).is_err());
    assert!(inspect.validate_for(&OwnerOperation::OwnerStatus).is_err());
    assert!(ContextResponse::Outcome(outcome()).validate_for(&set).is_ok());
    let mut wrong = outcome();
    wrong.request_id = "other".into();
    assert!(ContextResponse::Outcome(wrong).validate_for(&set).is_err());
    let mut historical = outcome();
    historical.provider_id = "historical-provider".into();
    assert!(ContextResponse::Outcome(historical.clone()).validate_for(&set).is_err());
    let status = OwnerOperation::ContextRequestStatus(ContextRequestStatusRequest { request_id: historical.request_id.clone() });
    assert!(ContextResponse::Outcome(historical).validate_for(&status).is_ok());
    let mut error = ContextError { code: ContextErrorCode::PhasePersistence, partial: Some(outcome()) };
    assert!(ContextResponse::Error(error.clone()).validate_for(&set).is_err());
    error.partial.as_mut().unwrap().phase = ContextPhase::ProjectedUnverified;
    assert!(ContextResponse::Error(error.clone()).validate_for(&set).is_ok());
    assert!(ContextResponse::Error(error.clone()).validate_for(&OwnerOperation::ContextInspect).is_err());
    error.partial.as_mut().unwrap().request_id = "different-request".into();
    assert!(ContextResponse::Error(error).validate_for(&set).is_err());
    let mut bad = outcome();
    bad.failure = Some(ContextFailure::ReadbackFailed);
    assert!(ContextResponse::Outcome(bad).validate_for(&set).is_err());
    let mut bad = inspection();
    bad.model_context_window.source = ContextSource::Unspecified;
    assert!(ContextResponse::Inspect(bad).validate_for(&OwnerOperation::ContextInspect).is_err());
    let mut replay = outcome();
    replay.replay = true;
    replay.phase = ContextPhase::ProjectionStarted;
    assert!(ContextResponse::Outcome(replay.clone()).validate_for(&set).is_err());
    replay.phase = ContextPhase::ProjectionUncertain;
    assert!(ContextResponse::Outcome(replay.clone()).validate_for(&set).is_ok());
    replay.replay = false;
    assert!(ContextResponse::Outcome(replay).validate_for(&set).is_err());
}

struct Fixture {
    profile: ProfileIdentity,
    runtime: RuntimeDirectory,
    _root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new().prefix("ct-")
            .permissions(fs::Permissions::from_mode(0o700)).tempdir().unwrap();
        let profile_path = root.path().join("p");
        let runtime_path = root.path().join("r");
        fs::DirBuilder::new().mode(0o700).create(&profile_path).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&runtime_path).unwrap();
        Self {
            profile: ProfileIdentity::existing(profile_path).unwrap(),
            runtime: RuntimeDirectory::existing(runtime_path).unwrap(),
            _root: root,
        }
    }

    fn server(&self) -> OwnerServer {
        let owner = crate::owner::ProfileOwner::acquire(self.profile.clone()).unwrap();
        OwnerServer::bind(owner, self.runtime.clone()).unwrap()
    }

    async fn client(&self) -> OwnerClient {
        OwnerClient::connect(&self.profile, &self.runtime, 7).await.unwrap()
    }
}

fn run(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread().enable_io().enable_time().build().unwrap().block_on(async {
        timeout(IO_TIMEOUT + std::time::Duration::from_secs(3), future).await.unwrap();
    });
}

#[test]
fn synthetic_status_and_context_round_trips_use_separate_envelopes() {
    run(async {
        let fixture = Fixture::new();
        let server = fixture.server();
        let actor = tokio::spawn(async move {
            let pending = server.accept().await.unwrap();
            assert_eq!(pending.request().operation, OwnerOperation::OwnerStatus);
            pending.respond_status().await.unwrap();
        });
        assert!(fixture.client().await.owner_status().await.unwrap().profile_lease_held);
        actor.await.unwrap();

        let server = fixture.server();
        let actor = tokio::spawn(async move {
            let pending = server.accept().await.unwrap();
            assert_eq!(pending.request().operation, OwnerOperation::ContextInspect);
            pending.respond_context(ContextResponse::Inspect(inspection())).await.unwrap();
        });
        assert_eq!(fixture.client().await.context_inspect().await.unwrap(), inspection());
        actor.await.unwrap();

        let server = fixture.server();
        let actor = tokio::spawn(async move {
            let pending = server.accept().await.unwrap();
            assert_eq!(pending.request().operation, OwnerOperation::ContextSet(set_request()));
            pending.respond_context(ContextResponse::Outcome(outcome())).await.unwrap();
        });
        assert_eq!(fixture.client().await.context_set(set_request()).await.unwrap(), outcome());
        actor.await.unwrap();
    });
}

#[test]
fn unsupported_context_is_explicit_without_declaring_write_capability() {
    run(async {
        let fixture = Fixture::new();
        let server = fixture.server();
        let actor = tokio::spawn(async move {
            server.accept().await.unwrap().respond_context(ContextResponse::Error(ContextError {
                code: ContextErrorCode::Unsupported, partial: None,
            })).await.unwrap();
        });
        let result = fixture.client().await.context_set(set_request()).await;
        assert!(matches!(result, Err(ContextClientError::Rejected(ContextError { code: ContextErrorCode::Unsupported, partial: None }))));
        actor.await.unwrap();
    });
}

#[test]
fn status_query_accepts_historical_provider_but_not_another_request() {
    run(async {
        let fixture = Fixture::new();
        let server = fixture.server();
        let actor = tokio::spawn(async move {
            let mut historical = outcome();
            historical.provider_id = "historical-provider".into();
            historical.replay = true;
            server.accept().await.unwrap().respond_context(ContextResponse::Outcome(historical)).await.unwrap();
        });
        let historical = fixture.client().await.context_request_status(set_request().request_id).await.unwrap();
        assert_eq!(historical.provider_id, "historical-provider");
        assert!(historical.replay);
        actor.await.unwrap();
    });
}

#[test]
fn pending_request_rejects_wrong_response_api_and_kind() {
    run(async {
        let fixture = Fixture::new();
        let server = fixture.server();
        let actor = tokio::spawn(async move {
            assert!(matches!(server.accept().await.unwrap().respond_context(ContextResponse::Inspect(inspection())).await, Err(Error::InvalidMessage)));
        });
        assert!(fixture.client().await.owner_status().await.is_err());
        actor.await.unwrap();

        let server = fixture.server();
        let actor = tokio::spawn(async move {
            assert!(matches!(server.accept().await.unwrap().respond_status().await, Err(Error::InvalidMessage)));
        });
        assert!(matches!(fixture.client().await.context_set(set_request()).await, Err(ContextClientError::OutcomeUnknown)));
        actor.await.unwrap();
    });
}

#[test]
fn disconnect_after_set_is_unknown_and_does_not_retry() {
    run(async {
        let fixture = Fixture::new();
        let server = fixture.server();
        let actor = tokio::spawn(async move {
            let pending = server.accept().await.unwrap();
            assert_eq!(pending.request().operation, OwnerOperation::ContextSet(set_request()));
            // Dropping closes the stream; no application effect is simulated.
            drop(pending);
            assert!(timeout(std::time::Duration::from_millis(50), server.accept()).await.is_err());
        });
        assert!(matches!(fixture.client().await.context_set(set_request()).await, Err(ContextClientError::OutcomeUnknown)));
        actor.await.unwrap();
    });
}

#[test]
fn timeout_after_set_is_unknown_without_canceling_pending_request() {
    run(async {
        let fixture = Fixture::new();
        let server = fixture.server();
        let actor = tokio::spawn(async move {
            let pending = server.accept().await.unwrap();
            tokio::time::sleep(IO_TIMEOUT + std::time::Duration::from_millis(20)).await;
            pending.owner().ensure_current().unwrap();
            drop(pending);
        });
        assert!(matches!(fixture.client().await.context_set(set_request()).await, Err(ContextClientError::OutcomeUnknown)));
        actor.await.unwrap();
    });
}

#[test]
fn first_write_failure_is_unknown_but_local_invalid_request_is_not_sent() {
    run(async {
        let fixture = Fixture::new();
        let path = endpoint_path(&fixture.profile, &fixture.runtime).unwrap();
        let _listener = UnixListener::bind(&path).unwrap();
        let (stream, peer) = UnixStream::pair().unwrap();
        drop(peer);
        let client = OwnerClient {
            stream, profile: fixture.profile.clone(), runtime: fixture.runtime.clone(),
            socket_key: check_socket(&path).unwrap(), path: path.clone(), request_id: 7, epoch: [0; 16],
        };
        assert!(matches!(client.context_set(set_request()).await, Err(ContextClientError::OutcomeUnknown)));

        let (stream, mut peer) = UnixStream::pair().unwrap();
        let client = OwnerClient {
            stream, profile: fixture.profile.clone(), runtime: fixture.runtime.clone(),
            socket_key: check_socket(&path).unwrap(), path, request_id: 7, epoch: [0; 16],
        };
        let mut invalid = set_request();
        invalid.schema_version = 2;
        assert!(matches!(client.context_set(invalid).await, Err(ContextClientError::Owner(Error::InvalidMessage))));
        let mut received = Vec::new();
        peer.read_to_end(&mut received).await.unwrap();
        assert!(received.is_empty());
        assert!(matches!(encode_frame(&"x".repeat(MAX_FRAME_BYTES)), Err(Error::FrameTooLarge)));
    });
}

#[test]
fn malformed_or_misbound_response_after_set_never_becomes_success() {
    run(async {
        let fixture = Fixture::new();
        for case in ["id", "profile", "epoch", "version", "persistent_id", "provider", "kind", "duplicate", "oversize"] {
            let server = fixture.server();
            let actor = tokio::spawn(async move {
                let mut pending = server.accept().await.unwrap();
                let mut envelope = ContextEnvelope {
                    protocol_major: PROTOCOL_MAJOR, request_id: pending.request.request_id,
                    profile: pending.request.profile, owner_epoch: pending.request.owner_epoch,
                    response: ContextResponse::Outcome(outcome()),
                };
                match case {
                    "id" => envelope.request_id += 1,
                    "profile" => envelope.profile.inode ^= 1,
                    "epoch" => envelope.owner_epoch[0] ^= 1,
                    "version" => envelope.protocol_major = 2,
                    "persistent_id" => if let ContextResponse::Outcome(value) = &mut envelope.response { value.request_id = "other".into(); },
                    "provider" => if let ContextResponse::Outcome(value) = &mut envelope.response { value.provider_id = "other".into(); },
                    "kind" => envelope.response = ContextResponse::Inspect(inspection()),
                    "duplicate" => {
                        let bytes = serde_json::to_string(&envelope).unwrap().replacen('{', r#"{"request_id":7,"#, 1).into_bytes();
                        write_encoded_frame(&mut pending.stream, &bytes).await.unwrap();
                        return;
                    }
                    "oversize" => {
                        pending.stream.write_all(&((MAX_FRAME_BYTES + 1) as u32).to_be_bytes()).await.unwrap();
                        return;
                    }
                    _ => unreachable!(),
                }
                // Test-only access intentionally bypasses the typed responder.
                write_frame(&mut pending.stream, &envelope).await.unwrap();
            });
            assert!(matches!(fixture.client().await.context_set(set_request()).await, Err(ContextClientError::OutcomeUnknown)), "case {case}");
            actor.await.unwrap();
        }
    });
}

#[test]
fn server_rejects_invalid_requests_without_reflecting_input() {
    run(async {
        let fixture = Fixture::new();
        for case in ["empty_patch", "schema", "duplicate", "unreviewed", "outer_duplicate"] {
            let server = fixture.server();
            let actor = tokio::spawn(async move {
                assert!(matches!(server.accept().await, Err(Error::InvalidMessage)));
            });
            let path = endpoint_path(&fixture.profile, &fixture.runtime).unwrap();
            let mut stream = UnixStream::connect(path).await.unwrap();
            write_frame(&mut stream, &Hello { protocol_major: PROTOCOL_MAJOR, request_id: 7, profile: fixture.profile.key() }).await.unwrap();
            let ack: HelloAck = read_frame(&mut stream).await.unwrap();
            let mut request = set_request();
            if case == "empty_patch" {
                request.patch.model_context_window = None;
                request.patch.model_auto_compact_token_limit = None;
            }
            if case == "schema" { request.schema_version = 2; }
            let mut bytes = serde_json::to_string(&OwnerRequest {
                protocol_major: PROTOCOL_MAJOR, request_id: 7, profile: fixture.profile.key(),
                owner_epoch: ack.owner_epoch, operation: OwnerOperation::ContextSet(request),
            }).unwrap();
            match case {
                "duplicate" => bytes = bytes.replace(r#""patch":{"#, r#""patch":{"model_context_window":null,"#),
                "unreviewed" => bytes = bytes.replace(r#""patch":{"#, r#""patch":{"rawValue":"secret-do-not-echo","#),
                "outer_duplicate" => bytes = bytes.replacen('{', r#"{"request_id":7,"#, 1),
                _ => (),
            }
            write_encoded_frame(&mut stream, bytes.as_bytes()).await.unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).await.unwrap();
            assert!(response.is_empty());
            actor.await.unwrap();
        }
    });
}

#[test]
fn capabilities_have_a_closed_separate_wire_without_changing_status() {
    assert_eq!(serde_json::to_string(&OwnerOperation::OwnerCapabilities).unwrap(), r#""owner_capabilities""#);
    assert_eq!(serde_json::to_string(&OwnerCapability::StatusOnly).unwrap(), r#""status_only""#);
    assert_eq!(serde_json::to_string(&OwnerCapability::ContextSetReady).unwrap(), r#""context_set_ready""#);
    for wire in [r#""unknown""#, r#"true"#, r#"null"#, r#"{"context_set_ready":{"auth":"secret"}}"#] {
        assert!(serde_json::from_str::<OwnerCapability>(wire).is_err());
    }
    let response = OwnerCapabilitiesResponse {
        protocol_major: PROTOCOL_MAJOR, request_id: 7, profile: ProfileKey { device: 1, inode: 2 },
        owner_epoch: [0; 16], capability: OwnerCapability::ContextSetReady,
    };
    let wire = serde_json::to_string(&response).unwrap();
    assert!(serde_json::from_str::<OwnerResponse>(&wire).is_err());
    assert!(serde_json::from_str::<ContextEnvelope>(&wire).is_err());
    assert!(serde_json::from_str::<OwnerCapabilitiesResponse>(&wire.replace(r#""capability":"#, r#""extra":true,"capability":"#)).is_err());
    assert!(serde_json::from_str::<OwnerCapabilitiesResponse>(&wire.replace(r#""capability":"#, r#""capability":"status_only","capability":"#)).is_err());
    assert!(ContextResponse::Error(ContextError { code: ContextErrorCode::Unsupported, partial: None })
        .validate_for(&OwnerOperation::OwnerCapabilities).is_err());
    // This snapshot remains the original owner-status representation, not an
    // alias to or fallback for capability discovery.
    legacy_status_wire_and_major_remain_exact();
}

#[test]
fn synthetic_capability_discovery_returns_only_explicit_session_selection() {
    run(async {
        let fixture = Fixture::new();
        for capability in [OwnerCapability::StatusOnly, OwnerCapability::ContextSetReady] {
            let server = fixture.server();
            let actor = tokio::spawn(async move {
                let pending = server.accept().await.unwrap();
                assert_eq!(pending.request().operation, OwnerOperation::OwnerCapabilities);
                pending.respond_capabilities(capability).await.unwrap();
            });
            assert_eq!(fixture.client().await.owner_capabilities().await.unwrap(), capability);
            actor.await.unwrap();
        }
    });
}

#[test]
fn capabilities_reject_wrong_responder_for_the_received_operation() {
    run(async {
        let fixture = Fixture::new();
        let server = fixture.server();
        let actor = tokio::spawn(async move {
            assert!(matches!(server.accept().await.unwrap().respond_capabilities(OwnerCapability::ContextSetReady).await, Err(Error::InvalidMessage)));
        });
        assert!(fixture.client().await.owner_status().await.is_err());
        actor.await.unwrap();

        for use_context in [false, true] {
            let server = fixture.server();
            let actor = tokio::spawn(async move {
                let pending = server.accept().await.unwrap();
                let result = if use_context {
                    pending.respond_context(ContextResponse::Error(ContextError { code: ContextErrorCode::Unsupported, partial: None })).await
                } else {
                    pending.respond_status().await
                };
                assert!(matches!(result, Err(Error::InvalidMessage)));
            });
            assert!(fixture.client().await.owner_capabilities().await.is_err());
            actor.await.unwrap();
        }
    });
}

#[test]
fn unknown_legacy_missing_and_misbound_capabilities_never_default_to_ready() {
    run(async {
        let fixture = Fixture::new();
        for case in ["id", "profile", "epoch", "version", "unknown", "missing", "legacy", "disconnect"] {
            let server = fixture.server();
            let actor = tokio::spawn(async move {
                let mut pending = server.accept().await.unwrap();
                let mut response = OwnerCapabilitiesResponse {
                    protocol_major: PROTOCOL_MAJOR, request_id: pending.request.request_id,
                    profile: pending.request.profile, owner_epoch: pending.request.owner_epoch,
                    capability: OwnerCapability::ContextSetReady,
                };
                match case {
                    "id" => response.request_id += 1,
                    "profile" => response.profile.inode ^= 1,
                    "epoch" => response.owner_epoch[0] ^= 1,
                    "version" => response.protocol_major = 2,
                    "unknown" | "missing" => {
                        let wire = serde_json::to_string(&response).unwrap();
                        let wire = if case == "unknown" { wire.replace("context_set_ready", "unknown") }
                            else { wire.replace(",\"capability\":\"context_set_ready\"", "") };
                        write_encoded_frame(&mut pending.stream, wire.as_bytes()).await.unwrap();
                        return;
                    }
                    "legacy" => {
                        let legacy = OwnerResponse {
                            protocol_major: PROTOCOL_MAJOR, request_id: pending.request.request_id,
                            profile: pending.request.profile, owner_epoch: pending.request.owner_epoch,
                            status: OwnerStatus { profile_lease_held: true },
                        };
                        write_frame(&mut pending.stream, &legacy).await.unwrap();
                        return;
                    }
                    "disconnect" => return,
                    _ => unreachable!(),
                }
                write_frame(&mut pending.stream, &response).await.unwrap();
            });
            assert!(fixture.client().await.owner_capabilities().await.is_err(), "case {case}");
            actor.await.unwrap();
        }
    });
}
