// SPDX-License-Identifier: AGPL-3.0-or-later
//! Durable, bounded metadata for the Codex context operation, not a file writer.
//!
//! Look up the request before changing master data. Insert its SavedOnly entry
//! in the same caller-owned transaction as the master-data patch. Commit a new
//! ProjectionStarted transition before attempting any live-file write; failure
//! to persist that transition forbids the write. A replay never authorizes a
//! second write. An interrupted ProjectionStarted is uncertain, not rolled back.
//! This journal stores no raw configuration, auth, paths, or arbitrary errors.

use std::fmt;

use rusqlite::{Connection, Transaction};
use serde::{Deserialize, Serialize};

use crate::helpers::{db_get, db_patch_fields, db_put};
use crate::schema::DbTable;

const RECORD_VERSION: u32 = 1;
const TABLE: DbTable = DbTable::CodexContextOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    SavedOnly,
    ProjectionStarted,
    ProjectedUnverified,
    Verified,
    ProjectionUncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    ProjectionPreflightFailed,
    ProjectionOutcomeUnknown,
    ReadbackFailed,
    ReadbackMismatch,
}

/// A closed metadata schema with no raw config, auth, path or error-text field.
/// String syntax validation does not prove that a value is nonsensitive: the
/// trusted service must compute the hashes and resolve provider IDs. Never
/// deserialize an IPC-provided journal payload and persist it directly.
/// The request fingerprint must bind the operation/schema, profile, target,
/// provider, requested patch and expected revision, not only the numeric pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalInput {
    pub request_id: String,
    pub provider_id: String,
    pub request_fingerprint: String,
    pub expected_revision: String,
    pub planned_live_digest: String,
    pub context: Option<i64>,
    pub auto_compact: Option<i64>,
    pub master_changed: bool,
    pub live_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalEntry {
    pub input: JournalInput,
    pub phase: Phase,
    pub failure: Option<FailureCode>,
}

impl JournalEntry {
    /// A persisted start marker does not prove whether an interrupted rename
    /// happened. Reading a replay never resumes or repairs that operation.
    pub fn replay_phase(&self) -> Phase {
        match self.phase {
            Phase::ProjectionStarted => Phase::ProjectionUncertain,
            phase => phase,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Missing,
    Replay(JournalEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertResult {
    Inserted(JournalEntry),
    Replay(JournalEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvanceResult {
    Advanced(JournalEntry),
    Replay(JournalEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    InvalidId,
    InvalidDigest,
    InvalidValues,
    InvalidRecord,
    UnsupportedVersion,
    FingerprintConflict,
    NotFound,
    StalePhase,
    InvalidTransition,
    Storage(String),
}

impl fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Stable diagnostics never echo caller IDs, digests or database text.
        formatter.write_str(match self {
            Self::InvalidId => "CODEX_JOURNAL_INVALID_ID",
            Self::InvalidDigest => "CODEX_JOURNAL_INVALID_DIGEST",
            Self::InvalidValues => "CODEX_JOURNAL_INVALID_VALUES",
            Self::InvalidRecord => "CODEX_JOURNAL_INVALID_RECORD",
            Self::UnsupportedVersion => "CODEX_JOURNAL_UNSUPPORTED_VERSION",
            Self::FingerprintConflict => "CODEX_JOURNAL_FINGERPRINT_CONFLICT",
            Self::NotFound => "CODEX_JOURNAL_NOT_FOUND",
            Self::StalePhase => "CODEX_JOURNAL_STALE_PHASE",
            Self::InvalidTransition => "CODEX_JOURNAL_INVALID_TRANSITION",
            Self::Storage(_) => "CODEX_JOURNAL_STORAGE_ERROR",
        })
    }
}

impl std::error::Error for JournalError {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRecord {
    version: u32,
    entry: JournalEntry,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn check_id(value: &str) -> Result<(), JournalError> {
    if valid_id(value) {
        Ok(())
    } else {
        Err(JournalError::InvalidId)
    }
}

fn check_digest(value: &str) -> Result<(), JournalError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(JournalError::InvalidDigest)
    }
}

fn validate_input(input: &JournalInput) -> Result<(), JournalError> {
    check_id(&input.request_id)?;
    check_id(&input.provider_id)?;
    check_digest(&input.request_fingerprint)?;
    check_digest(&input.expected_revision)?;
    check_digest(&input.planned_live_digest)?;
    if input.context.is_some_and(|value| value <= 0)
        || input.auto_compact.is_some_and(|value| value <= 0)
        || matches!(
            (input.context, input.auto_compact),
            (Some(context), Some(limit)) if limit > context
        )
        || (input.context.is_none() && input.auto_compact.is_none())
    {
        return Err(JournalError::InvalidValues);
    }
    Ok(())
}

fn valid_failure(input: &JournalInput, phase: Phase, failure: Option<FailureCode>) -> bool {
    match (phase, failure) {
        (
            Phase::SavedOnly,
            Some(FailureCode::ReadbackFailed | FailureCode::ReadbackMismatch),
        ) => !input.live_changed,
        (Phase::SavedOnly, None | Some(FailureCode::ProjectionPreflightFailed))
        | (Phase::ProjectionStarted, None)
        | (
            Phase::ProjectedUnverified,
            None | Some(FailureCode::ReadbackFailed | FailureCode::ReadbackMismatch),
        )
        | (Phase::Verified, None)
        | (Phase::ProjectionUncertain, Some(FailureCode::ProjectionOutcomeUnknown)) => true,
        _ => false,
    }
}

fn read_entry(conn: &Connection, id: &str) -> Result<Option<JournalEntry>, JournalError> {
    let Some(mut value) = db_get(conn, TABLE, id).map_err(JournalError::Storage)? else {
        return Ok(None);
    };
    // These are storage-helper metadata, not part of the closed journal schema.
    let object = value.as_object_mut().ok_or(JournalError::InvalidRecord)?;
    for key in ["id", "created_at", "updated_at"] {
        object.remove(key);
    }
    let record: StoredRecord =
        serde_json::from_value(value).map_err(|_| JournalError::InvalidRecord)?;
    if record.version != RECORD_VERSION {
        return Err(JournalError::UnsupportedVersion);
    }
    validate_input(&record.entry.input)?;
    if record.entry.input.request_id != id
        || !valid_failure(&record.entry.input, record.entry.phase, record.entry.failure)
    {
        return Err(JournalError::InvalidRecord);
    }
    Ok(Some(record.entry))
}

/// Must precede both revision checking and master-data mutation, so a completed
/// request can be replayed even though its own writes changed the revision.
/// Every Replay returns the recorded outcome only; it is not permission to
/// advance a phase or write files. Use replay_phase() for interrupted starts.
pub fn lookup(
    conn: &Connection,
    id: &str,
    request_fingerprint: &str,
) -> Result<Lookup, JournalError> {
    check_id(id)?;
    check_digest(request_fingerprint)?;
    match read_entry(conn, id)? {
        None => Ok(Lookup::Missing),
        Some(entry) if entry.input.request_fingerprint == request_fingerprint => {
            Ok(Lookup::Replay(entry))
        }
        Some(_) => Err(JournalError::FingerprintConflict),
    }
}

/// Insert alongside the master-data patch in one caller-owned transaction.
/// A replay must not be accompanied by a second master-data mutation.
pub fn insert_saved(
    tx: &Transaction<'_>,
    input: JournalInput,
) -> Result<InsertResult, JournalError> {
    validate_input(&input)?;
    if let Lookup::Replay(entry) = lookup(tx, &input.request_id, &input.request_fingerprint)? {
        if entry.input != input {
            return Err(JournalError::FingerprintConflict);
        }
        return Ok(InsertResult::Replay(entry));
    }
    let entry = JournalEntry {
        input,
        phase: Phase::SavedOnly,
        failure: None,
    };
    let record = StoredRecord {
        version: RECORD_VERSION,
        entry: entry.clone(),
    };
    let value = serde_json::to_value(record).map_err(|_| JournalError::InvalidRecord)?;
    db_put(tx, TABLE, &entry.input.request_id, &value).map_err(JournalError::Storage)?;
    Ok(InsertResult::Inserted(entry))
}

/// Advance monotonically in a caller-owned transaction. Only Advanced followed
/// by a successful transaction commit is a new projection-start permit. Replay
/// is never permission to repeat a file write. Failure markers are terminal.
pub fn advance(
    tx: &Transaction<'_>,
    id: &str,
    request_fingerprint: &str,
    expected_phase: Phase,
    next_phase: Phase,
    failure: Option<FailureCode>,
) -> Result<AdvanceResult, JournalError> {
    let Lookup::Replay(mut entry) = lookup(tx, id, request_fingerprint)? else {
        return Err(JournalError::NotFound);
    };
    if entry.phase == next_phase && entry.failure == failure {
        return Ok(AdvanceResult::Replay(entry));
    }
    if entry.phase != expected_phase {
        return Err(JournalError::StalePhase);
    }
    if entry.failure.is_some() || !valid_failure(&entry.input, next_phase, failure) {
        return Err(JournalError::InvalidTransition);
    }
    let allowed = match (entry.phase, next_phase) {
        (Phase::SavedOnly, Phase::ProjectionStarted)
        | (Phase::ProjectionStarted, Phase::ProjectedUnverified)
        | (Phase::ProjectionStarted, Phase::ProjectionUncertain)
        | (Phase::ProjectedUnverified, Phase::Verified) => true,
        // The service must still read back and verify an unchanged live file.
        (Phase::SavedOnly, Phase::Verified) => !entry.input.live_changed,
        (current, next) if current == next => failure.is_some(),
        _ => false,
    };
    if !allowed {
        return Err(JournalError::InvalidTransition);
    }
    entry.phase = next_phase;
    entry.failure = failure;
    let phase = serde_json::to_value(next_phase).map_err(|_| JournalError::InvalidRecord)?;
    let failure = serde_json::to_value(failure).map_err(|_| JournalError::InvalidRecord)?;
    db_patch_fields(
        tx,
        TABLE,
        id,
        &[("entry.phase", phase), ("entry.failure", failure)],
    )
    .map_err(JournalError::Storage)?
    .ok_or(JournalError::NotFound)?;
    Ok(AdvanceResult::Advanced(entry))
}
