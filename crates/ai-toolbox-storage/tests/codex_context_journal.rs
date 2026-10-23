use std::sync::{Arc, Mutex};

use ai_toolbox_storage::change_hook::install_change_recorder;
use ai_toolbox_storage::codex_context_journal::{
    advance, insert_saved, lookup, AdvanceResult, FailureCode, InsertResult, JournalEntry,
    JournalError, JournalInput, Lookup, Phase,
};
use ai_toolbox_storage::helpers::{db_count, db_get, db_patch_fields, db_put, db_transaction};
use ai_toolbox_storage::schema::DbTable;
use ai_toolbox_storage::SqliteDbState;
use rusqlite::Transaction;
use serde_json::json;

fn input() -> JournalInput {
    JournalInput {
        request_id: "synthetic-request".to_string(),
        provider_id: "synthetic-provider".to_string(),
        request_fingerprint: "a".repeat(64),
        expected_revision: "b".repeat(64),
        planned_live_digest: "c".repeat(64),
        context: Some(1_000_000),
        auto_compact: Some(900_000),
        master_changed: true,
        live_changed: true,
    }
}

fn transact<T>(
    state: &SqliteDbState,
    operation: impl FnOnce(&Transaction<'_>) -> Result<T, JournalError>,
) -> Result<T, String> {
    state.with_conn_mut(|conn| {
        db_transaction(conn, |tx| operation(tx).map_err(|error| error.to_string()))
    })
}

fn read(state: &SqliteDbState) -> JournalEntry {
    state
        .with_conn(|conn| {
            let request = input();
            let Lookup::Replay(entry) = lookup(conn, &request.request_id, &request.request_fingerprint)
                .map_err(|error| error.to_string())?
            else {
                return Err("synthetic request missing".to_string());
            };
            Ok(entry)
        })
        .unwrap()
}

fn change(
    state: &SqliteDbState,
    expected: Phase,
    next: Phase,
    failure: Option<FailureCode>,
) -> Result<AdvanceResult, String> {
    transact(state, |tx| {
        advance(
            tx,
            &input().request_id,
            &input().request_fingerprint,
            expected,
            next,
            failure,
        )
    })
}

#[test]
fn interrupted_projection_is_durable_and_replay_is_uncertain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.db");
    let state = SqliteDbState::open(path.clone()).unwrap();
    assert!(matches!(
        transact(&state, |tx| insert_saved(tx, input())).unwrap(),
        InsertResult::Inserted(_)
    ));
    assert!(matches!(
        change(&state, Phase::SavedOnly, Phase::ProjectionStarted, None).unwrap(),
        AdvanceResult::Advanced(_)
    ));
    drop(state);
    let reopened = SqliteDbState::open(path).unwrap();
    let entry = read(&reopened);
    assert_eq!(entry.phase, Phase::ProjectionStarted);
    assert_eq!(entry.replay_phase(), Phase::ProjectionUncertain);
    assert_eq!(entry.input, input());
    assert!(matches!(
        change(&reopened, Phase::SavedOnly, Phase::ProjectionStarted, None).unwrap(),
        AdvanceResult::Replay(_)
    ));
}

#[test]
fn matching_replay_does_not_write_and_mismatched_fingerprint_fails() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    transact(&state, |tx| insert_saved(tx, input())).unwrap();
    let changes = Arc::new(Mutex::new(Vec::new()));
    state
        .with_conn(|conn| install_change_recorder(conn, changes.clone()))
        .unwrap();
    let before = read(&state);
    assert!(matches!(
        transact(&state, |tx| insert_saved(tx, input())).unwrap(),
        InsertResult::Replay(_)
    ));
    assert!(matches!(
        change(&state, Phase::SavedOnly, Phase::SavedOnly, None).unwrap(),
        AdvanceResult::Replay(_)
    ));
    state
        .with_conn(|conn| {
            assert_eq!(
                lookup(conn, &input().request_id, &"d".repeat(64)),
                Err(JournalError::FingerprintConflict)
            );
            Ok(())
        })
        .unwrap();
    let mut changed_metadata = input();
    changed_metadata.provider_id = "different-provider".to_string();
    assert_eq!(
        transact(&state, |tx| insert_saved(tx, changed_metadata)),
        Err(JournalError::FingerprintConflict.to_string())
    );
    assert_eq!(read(&state), before);
    assert!(changes.lock().unwrap().is_empty());
}

#[test]
fn invalid_identifiers_digests_and_numbers_never_create_entries() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    let mut invalid = Vec::new();
    for id in [String::new(), "x".repeat(129), "bad\nvalue".to_string()] {
        let mut request = input();
        request.request_id = id.clone();
        invalid.push((request, JournalError::InvalidId));
        let mut request = input();
        request.provider_id = id;
        invalid.push((request, JournalError::InvalidId));
    }
    for digest in ["a".repeat(63), "g".repeat(64), "A".repeat(64)] {
        let mut request = input();
        request.request_fingerprint = digest.clone();
        invalid.push((request, JournalError::InvalidDigest));
        let mut request = input();
        request.expected_revision = digest.clone();
        invalid.push((request, JournalError::InvalidDigest));
        let mut request = input();
        request.planned_live_digest = digest;
        invalid.push((request, JournalError::InvalidDigest));
    }
    for (context, compact) in [
        (Some(0), Some(1)),
        (Some(-1), None),
        (None, Some(0)),
        (Some(10), Some(11)),
        (None, None),
    ] {
        let mut request = input();
        request.context = context;
        request.auto_compact = compact;
        invalid.push((request, JournalError::InvalidValues));
    }
    for (request, expected) in invalid {
        assert_eq!(
            transact(&state, |tx| insert_saved(tx, request)),
            Err(expected.to_string())
        );
    }
    state
        .with_conn(|conn| {
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn valid_phase_progression_rejects_backwards_and_stale_updates() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    transact(&state, |tx| insert_saved(tx, input())).unwrap();
    assert_eq!(
        change(&state, Phase::SavedOnly, Phase::Verified, None),
        Err(JournalError::InvalidTransition.to_string())
    );
    change(&state, Phase::SavedOnly, Phase::ProjectionStarted, None).unwrap();
    assert_eq!(
        change(&state, Phase::SavedOnly, Phase::ProjectedUnverified, None),
        Err(JournalError::StalePhase.to_string())
    );
    change(&state, Phase::ProjectionStarted, Phase::ProjectedUnverified, None).unwrap();
    change(&state, Phase::ProjectedUnverified, Phase::Verified, None).unwrap();
    assert_eq!(read(&state).replay_phase(), Phase::Verified);
    assert_eq!(
        change(&state, Phase::Verified, Phase::SavedOnly, None),
        Err(JournalError::InvalidTransition.to_string())
    );
    assert_eq!(
        change(&state, Phase::Verified, Phase::ProjectionStarted, None),
        Err(JournalError::InvalidTransition.to_string())
    );
}

#[test]
fn no_file_change_can_be_verified_without_claiming_a_write() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    let mut request = input();
    request.master_changed = false;
    request.live_changed = false;
    transact(&state, |tx| insert_saved(tx, request)).unwrap();
    change(&state, Phase::SavedOnly, Phase::Verified, None).unwrap();
    let entry = read(&state);
    assert_eq!(entry.phase, Phase::Verified);
    assert!(!entry.input.live_changed);
    assert!(!entry.input.master_changed);

    for failure in [FailureCode::ReadbackFailed, FailureCode::ReadbackMismatch] {
        let state = SqliteDbState::in_memory_for_test().unwrap();
        let mut request = input();
        request.live_changed = false;
        transact(&state, |tx| insert_saved(tx, request)).unwrap();
        change(&state, Phase::SavedOnly, Phase::SavedOnly, Some(failure)).unwrap();
        let entry = read(&state);
        assert_eq!(entry.replay_phase(), Phase::SavedOnly);
        assert_eq!(entry.failure, Some(failure));
        assert!(matches!(
            change(&state, Phase::SavedOnly, Phase::SavedOnly, Some(failure)).unwrap(),
            AdvanceResult::Replay(_)
        ));
        assert_eq!(
            change(&state, Phase::SavedOnly, Phase::Verified, None),
            Err(JournalError::InvalidTransition.to_string())
        );

        let changed_live = SqliteDbState::in_memory_for_test().unwrap();
        transact(&changed_live, |tx| insert_saved(tx, input())).unwrap();
        assert_eq!(
            change(&changed_live, Phase::SavedOnly, Phase::SavedOnly, Some(failure)),
            Err(JournalError::InvalidTransition.to_string())
        );
        assert_eq!(read(&changed_live).failure, None);
    }
}

#[test]
fn uncertain_projection_is_terminal_and_cannot_be_called_saved_only() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    transact(&state, |tx| insert_saved(tx, input())).unwrap();
    change(&state, Phase::SavedOnly, Phase::ProjectionStarted, None).unwrap();
    assert_eq!(
        change(&state, Phase::ProjectionStarted, Phase::ProjectionUncertain, None),
        Err(JournalError::InvalidTransition.to_string())
    );
    change(
        &state,
        Phase::ProjectionStarted,
        Phase::ProjectionUncertain,
        Some(FailureCode::ProjectionOutcomeUnknown),
    )
    .unwrap();
    assert_eq!(
        change(&state, Phase::ProjectionUncertain, Phase::SavedOnly, None),
        Err(JournalError::InvalidTransition.to_string())
    );
    assert_eq!(
        change(&state, Phase::ProjectionUncertain, Phase::ProjectionStarted, None),
        Err(JournalError::InvalidTransition.to_string())
    );
    assert_eq!(read(&state).phase, Phase::ProjectionUncertain);
}

#[test]
fn closed_failures_can_be_recorded_but_never_cleared_for_an_implicit_retry() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    transact(&state, |tx| insert_saved(tx, input())).unwrap();
    change(
        &state,
        Phase::SavedOnly,
        Phase::SavedOnly,
        Some(FailureCode::ProjectionPreflightFailed),
    )
    .unwrap();
    assert!(matches!(
        change(
            &state,
            Phase::SavedOnly,
            Phase::SavedOnly,
            Some(FailureCode::ProjectionPreflightFailed)
        )
        .unwrap(),
        AdvanceResult::Replay(_)
    ));
    assert_eq!(
        change(&state, Phase::SavedOnly, Phase::ProjectionStarted, None),
        Err(JournalError::InvalidTransition.to_string())
    );
    assert_eq!(
        change(&state, Phase::SavedOnly, Phase::SavedOnly, None),
        Err(JournalError::InvalidTransition.to_string())
    );

    for failure in [FailureCode::ReadbackFailed, FailureCode::ReadbackMismatch] {
        let state = SqliteDbState::in_memory_for_test().unwrap();
        transact(&state, |tx| insert_saved(tx, input())).unwrap();
        change(&state, Phase::SavedOnly, Phase::ProjectionStarted, None).unwrap();
        change(&state, Phase::ProjectionStarted, Phase::ProjectedUnverified, None).unwrap();
        change(
            &state,
            Phase::ProjectedUnverified,
            Phase::ProjectedUnverified,
            Some(failure),
        )
        .unwrap();
        assert_eq!(read(&state).failure, Some(failure));
        assert_eq!(
            change(&state, Phase::ProjectedUnverified, Phase::Verified, None),
            Err(JournalError::InvalidTransition.to_string())
        );
    }
}

#[test]
fn master_data_and_journal_insert_roll_back_in_the_same_transaction() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    state
        .with_conn(|conn| {
            db_put(
                conn,
                DbTable::CodexCommonConfig,
                "common",
                &json!({"config": "model_context_window=10\n", "unknown": true}),
            )
        })
        .unwrap();
    let result: Result<(), String> = state.with_conn_mut(|conn| {
        db_transaction(conn, |tx| {
            assert_eq!(
                lookup(tx, &input().request_id, &input().request_fingerprint)
                    .map_err(|error| error.to_string())?,
                Lookup::Missing
            );
            db_patch_fields(
                tx,
                DbTable::CodexCommonConfig,
                "common",
                &[("config", json!("model_context_window=1000000\n"))],
            )?;
            insert_saved(tx, input()).map_err(|error| error.to_string())?;
            Err("synthetic failure after both writes".to_string())
        })
    });
    assert!(result.is_err());
    state
        .with_conn(|conn| {
            let common = db_get(conn, DbTable::CodexCommonConfig, "common")?.unwrap();
            assert_eq!(common["config"], "model_context_window=10\n");
            assert_eq!(common["unknown"], true);
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn failed_projection_start_transaction_never_grants_a_durable_start() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    transact(&state, |tx| insert_saved(tx, input())).unwrap();
    let result: Result<(), String> = state.with_conn_mut(|conn| {
        db_transaction(conn, |tx| {
            advance(
                tx,
                &input().request_id,
                &input().request_fingerprint,
                Phase::SavedOnly,
                Phase::ProjectionStarted,
                None,
            )
            .map_err(|error| error.to_string())?;
            Err("synthetic failure before commit".to_string())
        })
    });
    assert!(result.is_err());
    assert_eq!(read(&state).phase, Phase::SavedOnly);
    // The file writer belongs to the caller; no writer may run on this result.
}

#[test]
fn stored_schema_rejects_unknown_payloads_versions_and_invalid_phases() {
    for (field, value, expected) in [
        (
            "config",
            json!("synthetic TOML is not journal data"),
            JournalError::InvalidRecord,
        ),
        ("version", json!(2), JournalError::UnsupportedVersion),
        ("entry.phase", json!("unknown_phase"), JournalError::InvalidRecord),
        ("entry.failure", json!("arbitrary error"), JournalError::InvalidRecord),
        ("entry.failure", json!("readback_failed"), JournalError::InvalidRecord),
        ("entry.input.context", json!(u64::MAX), JournalError::InvalidRecord),
    ] {
        let state = SqliteDbState::in_memory_for_test().unwrap();
        transact(&state, |tx| insert_saved(tx, input())).unwrap();
        state
            .with_conn(|conn| {
                db_patch_fields(
                    conn,
                    DbTable::CodexContextOperation,
                    &input().request_id,
                    &[(field, value)],
                )?;
                assert_eq!(
                    lookup(conn, &input().request_id, &input().request_fingerprint),
                    Err(expected)
                );
                Ok(())
            })
            .unwrap();
    }
}
