//! Explicit typed adapter for an already owned, already initialized database.
//! This does not start a server, establish desktop writer admission, or enable
//! a GUI capability. Linux is the only implemented service/transport platform.

use ai_toolbox_storage::SqliteDbState;

use crate::{
    owner::OwnerPermit,
    transport::{ContextError, ContextErrorCode, ContextResponse, PendingRequest},
};

#[cfg(target_os = "linux")]
use {
    ai_toolbox_codex_config::plan::{ContextPatch, Source, Target},
    ai_toolbox_storage::codex_context_journal::{FailureCode, Phase},
    crate::{
        codex_service::{self, ContextRequest, ContextService, ContextStatus, ServiceError},
        transport::{
            ContextFailure, ContextInspection, ContextOutcome, ContextOverride, ContextPhase,
            ContextSource, ContextTarget, OwnerOperation,
        },
    },
};

/// No database opener or unauthenticated operation entry is exposed here.
/// The caller must acquire the owner before opening the shared database and
/// close the full application writer admission map before serving requests.
pub struct ContextSession {
    // Drop the service's DB reference before this additional owner reference.
    // The shared DB also retains its lease until the original Connection closes.
    #[cfg(target_os = "linux")]
    service: ContextService,
    #[cfg(target_os = "linux")]
    owner: OwnerPermit,
    #[cfg(not(target_os = "linux"))]
    _private: (),
}

impl ContextSession {
    /// Reuse the exact existing shared Connection and owner allocation. No
    /// database open, migration, seed, lease acquisition or server bind occurs.
    /// Attachment cannot retroactively prove the required owner-before-open
    /// ordering. The retained Connection must not be replaced through trusted
    /// storage APIs; pathname/inode checks cannot prove original-Connection use.
    pub fn attach(
        owner: &OwnerPermit,
        db: &SqliteDbState,
    ) -> std::result::Result<Self, ContextError> {
        #[cfg(target_os = "linux")]
        {
            let service = ContextService::attach(owner, db).map_err(service_error)?;
            Ok(Self { service, owner: owner.clone() })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (owner, db);
            Err(context_error(ContextErrorCode::Unsupported))
        }
    }

    /// Execute only a transport-authenticated request bound to this live owner.
    /// This synchronous method rechecks endpoint/profile/epoch at execution.
    /// In an async host, move an Arc<Self> and the owned PendingRequest into
    /// spawn_blocking and retain both until the blocking work actually finishes;
    /// cancelling its waiter or disconnecting the socket is not cancellation of
    /// the operation. Return the pending request for its typed response method.
    ///
    /// No outer writer is acquired: the private service owns its non-reentrant
    /// gate and revalidates its original DB/file identity before each effect.
    /// Only Verified outcomes are success. An Error with partial: None means
    /// no verified outcome is attached, never that zero effects occurred. Query
    /// request status explicitly instead of automatically retrying a failed set.
    pub fn execute_pending(&self, pending: &PendingRequest) -> crate::Result<ContextResponse> {
        #[cfg(target_os = "linux")]
        {
            pending.require_context_owner(&self.owner)?;
            let response = match &pending.request().operation {
                OwnerOperation::OwnerStatus | OwnerOperation::OwnerCapabilities => {
                    return Err(crate::Error::InvalidMessage);
                }
                OwnerOperation::ContextInspect => self.service.inspect()
                    .map_err(service_error)
                    .and_then(inspection)
                    .map(ContextResponse::Inspect),
                OwnerOperation::ContextSet(request) => {
                    if request.validate().is_err() {
                        return Ok(ContextResponse::Error(context_error(ContextErrorCode::InvalidRequest)));
                    }
                    let operation = ContextRequest {
                        schema_version: request.schema_version,
                        request_id: request.request_id.clone(),
                        expected_provider_id: request.expected_provider_id.clone(),
                        expected_revision: request.expected_revision.clone(),
                        target: match request.target {
                            ContextTarget::Common => Target::Common,
                            ContextTarget::Provider => Target::Provider,
                        },
                        patch: ContextPatch {
                            model_context_window: request.patch.model_context_window,
                            model_auto_compact_token_limit: request.patch.model_auto_compact_token_limit,
                        },
                    };
                    self.service.execute(&operation).map_err(service_error).and_then(|status| {
                        // execute binds these identities through the existing
                        // fingerprint before returning a new or replayed status.
                        outcome(request.request_id.clone(), request.expected_provider_id.clone(), status)
                    }).map(ContextResponse::Outcome)
                }
                OwnerOperation::ContextRequestStatus(request) => {
                    if request.validate().is_err() {
                        return Ok(ContextResponse::Error(context_error(ContextErrorCode::InvalidRequest)));
                    }
                    self.service.request_status(&request.request_id).map_err(service_error)
                        .and_then(|record| record.ok_or_else(|| context_error(ContextErrorCode::RequestNotFound)))
                        .and_then(|record| outcome(record.request_id, record.provider_id, record.outcome))
                        .map(ContextResponse::Outcome)
                }
            };
            Ok(response.unwrap_or_else(ContextResponse::Error))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = pending;
            Err(crate::Error::UnsupportedPlatform)
        }
    }
}

fn context_error(code: ContextErrorCode) -> ContextError {
    // Do not look up by ID to invent a partial outcome: an ID conflict can
    // belong to a different fingerprint. None is not proof of zero effects.
    ContextError { code, partial: None }
}

#[cfg(target_os = "linux")]
fn service_error(error: ServiceError) -> ContextError {
    context_error(match error {
        ServiceError::Ownership => ContextErrorCode::Ownership,
        ServiceError::Storage => ContextErrorCode::Storage,
        ServiceError::InvalidMaster => ContextErrorCode::InvalidMaster,
        ServiceError::InvalidRequest => ContextErrorCode::InvalidRequest,
        ServiceError::ProviderChanged => ContextErrorCode::ProviderChanged,
        ServiceError::RevisionConflict => ContextErrorCode::RevisionConflict,
        ServiceError::RuntimeUnsupported => ContextErrorCode::RuntimeUnsupported,
        ServiceError::FileUnavailable => ContextErrorCode::FileUnavailable,
        ServiceError::JournalConflict => ContextErrorCode::JournalConflict,
        ServiceError::JournalInvalid => ContextErrorCode::JournalInvalid,
        ServiceError::PersistenceUncertain => ContextErrorCode::PersistenceUncertain,
        ServiceError::PhasePersistence { .. } => ContextErrorCode::PhasePersistence,
        ServiceError::Interrupted => ContextErrorCode::Interrupted,
    })
}

#[cfg(target_os = "linux")]
fn positive_value(value: Option<i64>, code: ContextErrorCode) -> std::result::Result<Option<u64>, ContextError> {
    value.map(|value| {
        u64::try_from(value).ok().filter(|value| *value != 0)
            .ok_or_else(|| context_error(code))
    }).transpose()
}

#[cfg(target_os = "linux")]
fn inspection(value: codex_service::ContextInspection) -> std::result::Result<ContextInspection, ContextError> {
    fn field(value: codex_service::InspectedOverride) -> std::result::Result<ContextOverride, ContextError> {
        Ok(ContextOverride {
            effective: positive_value(value.effective, ContextErrorCode::InvalidMaster)?,
            source: match value.source {
                Source::Common => ContextSource::Common,
                Source::Provider => ContextSource::Provider,
                Source::Unspecified => ContextSource::Unspecified,
            },
        })
    }
    Ok(ContextInspection {
        provider_id: value.provider_id,
        revision: value.revision,
        model_context_window: field(value.model_context_window)?,
        model_auto_compact_token_limit: field(value.model_auto_compact_token_limit)?,
    })
}

#[cfg(target_os = "linux")]
fn outcome(request_id: String, provider_id: String, status: ContextStatus) -> std::result::Result<ContextOutcome, ContextError> {
    Ok(ContextOutcome {
        request_id,
        provider_id,
        phase: match status.phase {
            Phase::SavedOnly => ContextPhase::SavedOnly,
            Phase::ProjectionStarted => ContextPhase::ProjectionStarted,
            Phase::ProjectedUnverified => ContextPhase::ProjectedUnverified,
            Phase::Verified => ContextPhase::Verified,
            Phase::ProjectionUncertain => ContextPhase::ProjectionUncertain,
        },
        failure: status.failure.map(|failure| match failure {
            FailureCode::ProjectionPreflightFailed => ContextFailure::ProjectionPreflightFailed,
            FailureCode::ProjectionOutcomeUnknown => ContextFailure::ProjectionOutcomeUnknown,
            FailureCode::ReadbackFailed => ContextFailure::ReadbackFailed,
            FailureCode::ReadbackMismatch => ContextFailure::ReadbackMismatch,
        }),
        replay: status.replay,
        model_context_window: positive_value(status.context, ContextErrorCode::JournalInvalid)?,
        model_auto_compact_token_limit: positive_value(status.auto_compact, ContextErrorCode::JournalInvalid)?,
    })
}

#[cfg(all(test, target_os = "linux"))]
#[path = "context_session_tests.rs"]
mod tests;
