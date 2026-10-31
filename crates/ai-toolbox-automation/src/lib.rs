//! Headless contract foundation with shared path-normalization and static gateway capability services.
//!
//! Schema version 1 is independent of desktop/package versions. Breaking wire
//! changes require a new schema version; consumers must reject unknown versions.
//! Discovery never resolves paths or initializes application state. The explicit
//! Linux owner client only inspects existing paths and connects to an owner;
//! it never creates paths/leases, opens a database, or starts a GUI/service owner.

use std::fmt::Write;
use std::path::PathBuf;

pub mod cli;
pub mod mcp;
pub const MAX_PATH_BYTES: usize = ai_toolbox_paths::PROTOCOL_INPUT_LIMIT;

pub const SCHEMA_VERSION: u32 = 1;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const HELP: &str = "AI Toolbox automation console (foundation only)\n\
Usage: ai-toolbox-cli [--format text|json] <command>\n\
Commands:\n\
  --help, -h       Show this help without initialization\n\
  --version, -V    Show automation package version\n\
  capabilities    Discover this foundation's static contract\n\
  codex status    Reserved typed operation; returns NOT_SUPPORTED\n\
  daemon status --profile <absolute-dir> --endpoint-dir <absolute-private-dir>\n\
                  Linux only; partial, lease-only owner status\n\
  codex context inspect <owner-options>\n\
                  Read merged master overrides, provider ID and revision\n\
  codex context set <owner-options> --request-id <id>\n\
      --expected-provider-id <id> --expected-revision <64-lowercase-hex>\n\
      --target common|provider [--model-context-window <integer>]\n\
      [--model-auto-compact-token-limit <integer>]\n\
                  Reserved: NOT_SUPPORTED; no owner is contacted\n\
  codex context request-status <owner-options> --request-id <id>\n\
                  Read recorded outcome only; never resume or repeat an effect\n\
Owner options: --profile <absolute-dir> --endpoint-dir <absolute-private-dir>\n\
No path, request ID, provider, revision or target defaults. IDs use 1..128 ASCII\n\
letters, digits, underscore or hyphen. Integers use decimal 1..9223372036854775807;\n\
when both are supplied, compact must not exceed context. Linux uses shared\n\
transport validation; unsupported platforms reject without owner or path I/O.\n\
Exit codes: 0 success, 2 INVALID_INPUT, 3 NOT_SUPPORTED, 4 invalid owner paths,\n\
5 OWNER_UNAVAILABLE, 6 owner protocol/identity rejection, 7 OWNER_TIMEOUT,\n\
8 CONTEXT_REJECTED, 9 CONTEXT_PARTIAL, 10 OUTCOME_UNKNOWN,\n\
70 owner client failure, 74 output I/O failure.\n\
JSON results go to stdout, including errors; text errors go to stderr.\n\
Only a verified context outcome succeeds. Partial or unknown results never\n\
automatically retry; explicitly query request-status using the same request ID.\n\
Inspect is not model defaults or live verification. Recorded verification does\n\
not establish current runtime state, client reload or backend context capacity.\n\
Status does not prove DB readiness or complete writer admission. No daemon is started.\n\
Context syntax is a Linux local client, not a claim that an owner enables writes.\n\
ContextSet is reserved and returns NOT_SUPPORTED before path or connection I/O.\n\
backup normalize-path --path <path> normalizes a supplied path without target file I/O.\n\
gateway supported-cli-keys queries static takeover support, not live status.\n\
mcp serve exposes the path service and static gateway query over stdio (MCP 2025-11-25).\n\
PID, start time, full daemon lifecycle and the P0 workflow remain incomplete.\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Help,
    Version,
    Capabilities,
    CodexStatus,
    BackupNormalizePath,
    GatewaySupportedCliKeys,
    DaemonStatus,
    ContextInspect,
    ContextSet,
    ContextRequestStatus,
}

impl Operation {
    pub const ALL: [Self; 10] = [
        Self::Help,
        Self::Version,
        Self::Capabilities,
        Self::CodexStatus,
        Self::BackupNormalizePath,
        Self::GatewaySupportedCliKeys,
        Self::DaemonStatus,
        Self::ContextInspect,
        Self::ContextSet,
        Self::ContextRequestStatus,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Help => "automation.help",
            Self::Version => "automation.version",
            Self::Capabilities => "automation.capabilities",
            Self::CodexStatus => "codex.status",
            Self::BackupNormalizePath => "backup.normalize_path",
            Self::GatewaySupportedCliKeys => "gateway.supported_cli_keys",
            Self::DaemonStatus => "daemon.status",
            Self::ContextInspect => "codex.context.inspect",
            Self::ContextSet => "codex.context.set",
            Self::ContextRequestStatus => "codex.context.request_status",
        }
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Help => "--help",
            Self::Version => "--version",
            Self::Capabilities => "capabilities",
            Self::CodexStatus => "codex status",
            Self::BackupNormalizePath => "backup normalize-path --path <path>",
            Self::GatewaySupportedCliKeys => "gateway supported-cli-keys",
            Self::DaemonStatus => "daemon status",
            Self::ContextInspect => "codex context inspect",
            Self::ContextSet => "codex context set",
            Self::ContextRequestStatus => "codex context request-status",
        }
    }

    /// Static availability, not merely the presence of local command syntax.
    /// Context owner readiness is deliberately not inferred by discovery.
    pub fn supported(self) -> bool {
        match self {
            Self::CodexStatus
            | Self::ContextInspect
            | Self::ContextSet
            | Self::ContextRequestStatus => false,
            Self::DaemonStatus => cfg!(target_os = "linux"),
            _ => true,
        }
    }
}

/// Closed command input. Paths cannot be constructed without pure validation.
/// There are no environment defaults or arbitrary invokes. Context input uses
/// the same closed transport DTO and validator as the existing owner client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Capabilities,
    CodexStatus,
    BackupNormalizePath(BackupPathInput),
    GatewaySupportedCliKeys,
    DaemonStatus(OwnerStatusInput),
    ContextInspect(OwnerStatusInput),
    ContextSet(ContextSetInput),
    ContextRequestStatus(ContextRequestStatusInput),
}

impl Command {
    fn operation(&self) -> Operation {
        match self {
            Self::Help => Operation::Help,
            Self::Version => Operation::Version,
            Self::Capabilities => Operation::Capabilities,
            Self::CodexStatus => Operation::CodexStatus,
            Self::BackupNormalizePath(_) => Operation::BackupNormalizePath,
            Self::GatewaySupportedCliKeys => Operation::GatewaySupportedCliKeys,
            Self::DaemonStatus(_) => Operation::DaemonStatus,
            Self::ContextInspect(_) => Operation::ContextInspect,
            Self::ContextSet(_) => Operation::ContextSet,
            Self::ContextRequestStatus(_) => Operation::ContextRequestStatus,
        }
    }
}

/// Adapter input bound, not a second path validator or safety policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupPathInput(String);
impl BackupPathInput {
    pub fn new(path: String) -> Result<Self, OperationError> {
        ai_toolbox_paths::validate_backup_path_input(&path)
            .map_err(|_| OperationError::invalid_input())?;
        Ok(Self(path))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerStatusInput {
    profile: PathBuf,
    endpoint_dir: PathBuf,
}

impl OwnerStatusInput {
    pub fn new(profile: PathBuf, endpoint_dir: PathBuf) -> Result<Self, OperationError> {
        if !profile.is_absolute() || !endpoint_dir.is_absolute() {
            return Err(OperationError::invalid_input());
        }
        Ok(Self {
            profile,
            endpoint_dir,
        })
    }
}

/// A validated closed write request. Owner paths never select a local database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextSetInput {
    owner: OwnerStatusInput,
    #[cfg(target_os = "linux")]
    request: ai_toolbox_management::transport::ContextSetRequest,
}

#[cfg(target_os = "linux")]
impl ContextSetInput {
    pub fn new(
        owner: OwnerStatusInput,
        request: ai_toolbox_management::transport::ContextSetRequest,
    ) -> Result<Self, OperationError> {
        request
            .validate()
            .map_err(|_| OperationError::invalid_input())?;
        Ok(Self { owner, request })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRequestStatusInput {
    owner: OwnerStatusInput,
    request_id: String,
}

impl ContextRequestStatusInput {
    pub fn new(owner: OwnerStatusInput, request_id: String) -> Result<Self, OperationError> {
        #[cfg(target_os = "linux")]
        ai_toolbox_management::transport::ContextRequestStatusRequest {
            request_id: request_id.clone(),
        }
        .validate()
        .map_err(|_| OperationError::invalid_input())?;
        Ok(Self { owner, request_id })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidInput,
    NotSupported,
    InvalidProfile,
    InvalidEndpoint,
    OwnerUnavailable,
    OwnerProtocolRejected,
    OwnerIdentityRejected,
    OwnerTimeout,
    OwnerClientFailed,
    ContextRejected,
    ContextPartial,
    OutcomeUnknown,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::NotSupported => "NOT_SUPPORTED",
            Self::InvalidProfile => "INVALID_PROFILE",
            Self::InvalidEndpoint => "INVALID_ENDPOINT",
            Self::OwnerUnavailable => "OWNER_UNAVAILABLE",
            Self::OwnerProtocolRejected => "OWNER_PROTOCOL_REJECTED",
            Self::OwnerIdentityRejected => "OWNER_IDENTITY_REJECTED",
            Self::OwnerTimeout => "OWNER_TIMEOUT",
            Self::OwnerClientFailed => "OWNER_CLIENT_FAILED",
            Self::ContextRejected => "CONTEXT_REJECTED",
            Self::ContextPartial => "CONTEXT_PARTIAL",
            Self::OutcomeUnknown => "OUTCOME_UNKNOWN",
        }
    }

    pub fn exit_code(self) -> u8 {
        match self {
            Self::InvalidInput => 2,
            Self::NotSupported => 3,
            Self::InvalidProfile | Self::InvalidEndpoint => 4,
            Self::OwnerUnavailable => 5,
            Self::OwnerProtocolRejected | Self::OwnerIdentityRejected => 6,
            Self::OwnerTimeout => 7,
            Self::OwnerClientFailed => 70,
            Self::ContextRejected => 8,
            Self::ContextPartial => 9,
            Self::OutcomeUnknown => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Parse,
    Dispatch,
    Inspect,
    Connect,
    Context,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Dispatch => "dispatch",
            Self::Inspect => "inspect",
            Self::Connect => "connect",
            Self::Context => "context",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationError {
    pub code: ErrorCode,
    pub stage: Stage,
    pub retryable: bool,
    // Static diagnostics never reflect raw arguments, which may contain secrets.
    pub message: &'static str,
}

impl OperationError {
    pub fn invalid_input() -> Self {
        Self {
            code: ErrorCode::InvalidInput,
            stage: Stage::Parse,
            retryable: false,
            message: "Invalid arguments. Use --help for the supported syntax.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Data {
    Help,
    Version,
    Capabilities,
    BackupPath(String),
    GatewayCliKeys(Vec<String>),
    OwnerStatus(OwnerStatusData),
    #[cfg(target_os = "linux")]
    ContextInspection(ai_toolbox_management::transport::ContextInspection),
    #[cfg(target_os = "linux")]
    ContextOutcome(ai_toolbox_management::transport::ContextOutcome),
    #[cfg(target_os = "linux")]
    ContextRejection(ai_toolbox_management::transport::ContextError),
}

/// Deliberately not database readiness, writer coverage, or business status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerStatusData {
    pub profile_lease_held: bool,
}

/// The result discriminant is the source of truth for both `ok` and exit code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub operation: Option<Operation>,
    /// Present only for a validated durable context request, never status correlation.
    pub request_id: Option<String>,
    /// Whitelisted context metadata for non-success results, not a success payload.
    pub partial: Option<Data>,
    pub result: Result<Data, OperationError>,
}

/// Closed, typed dispatch. There is intentionally no string-based invoke path.
/// Future business operations must use the existing extracted services, not
/// parallel SQL/config mergers or a mocked Tauri AppHandle.
pub fn execute(command: Command) -> Envelope {
    let operation = command.operation();
    #[cfg(target_os = "linux")]
    let request_id = match &command {
        Command::ContextSet(input) => Some(input.request.request_id.clone()),
        Command::ContextRequestStatus(input) => Some(input.request_id.clone()),
        _ => None,
    };
    #[cfg(not(target_os = "linux"))]
    let request_id = None;
    let mut partial = None;
    let result = match command {
        Command::Help => Ok(Data::Help),
        Command::Version => Ok(Data::Version),
        Command::Capabilities => Ok(Data::Capabilities),
        Command::CodexStatus => Err(OperationError {
            code: ErrorCode::NotSupported,
            stage: Stage::Dispatch,
            retryable: false,
            message: "Codex business services are not connected in this foundation.",
        }),
        Command::BackupNormalizePath(input) => {
            ai_toolbox_paths::normalize_backup_path_request(&input.0)
                .map(Data::BackupPath)
                .map_err(|error| OperationError {
                    code: ErrorCode::InvalidInput,
                    stage: Stage::Dispatch,
                    retryable: false,
                    message: error.message(),
                })
        }
        Command::GatewaySupportedCliKeys => Ok(Data::GatewayCliKeys(
            ai_toolbox_gateway_capabilities::supported_cli_keys(),
        )),
        Command::DaemonStatus(input) => owner_status(input).map(Data::OwnerStatus),
        command @ (Command::ContextInspect(_)
        | Command::ContextSet(_)
        | Command::ContextRequestStatus(_)) => match context_dispatch(command) {
            Ok(data) => Ok(data),
            Err((error, metadata)) => {
                partial = metadata;
                Err(error)
            }
        },
    };
    Envelope {
        operation: Some(operation),
        request_id,
        partial,
        result,
    }
}

#[cfg(not(target_os = "linux"))]
fn owner_status(_input: OwnerStatusInput) -> Result<OwnerStatusData, OperationError> {
    Err(OperationError {
        code: ErrorCode::NotSupported,
        stage: Stage::Dispatch,
        retryable: false,
        message: "Native owner-status isolation is supported only on Linux.",
    })
}

#[cfg(target_os = "linux")]
fn owner_status(input: OwnerStatusInput) -> Result<OwnerStatusData, OperationError> {
    use ai_toolbox_management::transport::OwnerClient;

    let (profile, endpoint) = existing_owner_paths(input)?;
    // This synchronous console boundary owns its runtime; never use this adapter
    // inside an async GUI chain. I/O and time remain enabled until the client drops.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| client_failure())?;
    runtime.block_on(async {
        // Exactly one request on a fresh connection: 1 is correlation within that
        // connection, not a durable request ID or an idempotency key. The public
        // envelope retains request_id=null; no persistent guarantee is invented.
        let client = OwnerClient::connect(&profile, &endpoint, 1)
            .await
            .map_err(owner_error)?;
        let status = client.owner_status().await.map_err(owner_error)?;
        Ok(OwnerStatusData {
            profile_lease_held: status.profile_lease_held,
        })
    })
}

#[cfg(target_os = "linux")]
fn existing_owner_paths(
    input: OwnerStatusInput,
) -> Result<
    (
        ai_toolbox_management::owner::ProfileIdentity,
        ai_toolbox_management::owner::RuntimeDirectory,
    ),
    OperationError,
> {
    use ai_toolbox_management::owner::{ProfileIdentity, RuntimeDirectory};
    // These existing-only helpers retain and recheck physical directory identity.
    // Do not substitute owner acquisition, storage initialization, or a fallback.
    let profile = ProfileIdentity::existing(&input.profile).map_err(|_| OperationError {
        code: ErrorCode::InvalidProfile,
        stage: Stage::Inspect,
        retryable: false,
        message: "The existing profile directory is unavailable, unsafe, or unsupported.",
    })?;
    let endpoint = RuntimeDirectory::existing(&input.endpoint_dir).map_err(|_| OperationError {
        code: ErrorCode::InvalidEndpoint,
        stage: Stage::Inspect,
        retryable: false,
        message: "The existing private endpoint directory is unavailable, unsafe, or unsupported.",
    })?;
    Ok((profile, endpoint))
}

#[cfg(target_os = "linux")]
fn client_failure() -> OperationError {
    OperationError {
        code: ErrorCode::OwnerClientFailed,
        stage: Stage::Connect,
        retryable: false,
        message: "The local owner-status client failed.",
    }
}

#[cfg(target_os = "linux")]
fn owner_error(error: ai_toolbox_management::Error) -> OperationError {
    use ai_toolbox_management::Error;
    let (code, retryable, message) = match error {
        Error::OwnerUnavailable => (
            ErrorCode::OwnerUnavailable,
            true,
            "The existing owner endpoint is unavailable; no owner was started.",
        ),
        Error::ProtocolMismatch | Error::InvalidMessage | Error::FrameTooLarge => (
            ErrorCode::OwnerProtocolRejected,
            false,
            "The owner protocol response was incompatible or invalid.",
        ),
        Error::PeerRejected
        | Error::StaleOwner
        | Error::IdentityChanged
        | Error::EndpointConflict
        | Error::InvalidProfile
        | Error::UnsafeDirectory => (
            ErrorCode::OwnerIdentityRejected,
            false,
            "The owner, profile, or endpoint identity could not be verified.",
        ),
        Error::TimedOut => (
            ErrorCode::OwnerTimeout,
            true,
            "The local owner-status request timed out.",
        ),
        // Never format raw I/O or protocol errors into public diagnostics.
        _ => return client_failure(),
    };
    OperationError {
        code,
        stage: Stage::Connect,
        retryable,
        message,
    }
}

type ContextDispatch = Result<Data, (OperationError, Option<Data>)>;

#[cfg(not(target_os = "linux"))]
fn context_dispatch(_command: Command) -> ContextDispatch {
    Err((
        OperationError {
            code: ErrorCode::NotSupported,
            stage: Stage::Dispatch,
            retryable: false,
            message: "The local context client is supported only on Linux; no owner was contacted.",
        },
        None,
    ))
}

#[cfg(target_os = "linux")]
fn context_dispatch(command: Command) -> ContextDispatch {
    use ai_toolbox_management::transport::{OwnerClient, OwnerOperation};
    let (paths, operation) = match command {
        Command::ContextInspect(paths) => (paths, OwnerOperation::ContextInspect),
        Command::ContextSet(_) => return Err((OperationError {
            code: ErrorCode::NotSupported,
            stage: Stage::Dispatch,
            retryable: false,
            message: "ContextSet is reserved until same-owner write capability is proven; no path was inspected and no owner was contacted.",
        }, None)),
        Command::ContextRequestStatus(input) => (input.owner, OwnerOperation::ContextRequestStatus(
            ai_toolbox_management::transport::ContextRequestStatusRequest { request_id: input.request_id }
        )),
        _ => return Err((OperationError::invalid_input(), None)),
    };
    let (profile, endpoint) = existing_owner_paths(paths).map_err(|error| (error, None))?;
    // Only the synchronous console owns this runtime. No database or service
    // owner is initialized; one connection performs one explicit operation.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| {
            (
                OperationError {
                    message:
                        "The local context owner client could not start; no operation was sent.",
                    ..client_failure()
                },
                None,
            )
        })?;
    runtime.block_on(async {
        let client = OwnerClient::connect(&profile, &endpoint, 1)
            .await
            .map_err(|error| (context_owner_error(error), None))?;
        match operation {
            OwnerOperation::ContextInspect => client
                .context_inspect()
                .await
                .map(Data::ContextInspection)
                .map_err(context_client_error),
            OwnerOperation::ContextSet(_) => {
                unreachable!("ContextSet is rejected before path or connection I/O")
            }
            OwnerOperation::ContextRequestStatus(request) => client
                .context_request_status(request.request_id)
                .await
                .map_err(context_client_error)
                .and_then(context_outcome),
            OwnerOperation::OwnerStatus | OwnerOperation::OwnerCapabilities => {
                unreachable!("closed context command dispatch")
            }
        }
    })
}

#[cfg(target_os = "linux")]
fn context_owner_error(error: ai_toolbox_management::Error) -> OperationError {
    let mut error = owner_error(error);
    // This is before a set write started, but never invite an automatic retry
    // from a generic consumer. The caller must explicitly choose its next action.
    error.retryable = false;
    error.message = match error.code {
        ErrorCode::OwnerTimeout => {
            "The context owner connection timed out before a confirmed response."
        }
        ErrorCode::OwnerClientFailed => {
            "The local context owner client failed; no write outcome is asserted."
        }
        _ => error.message,
    };
    error
}

#[cfg(target_os = "linux")]
fn context_client_error(
    error: ai_toolbox_management::transport::ContextClientError,
) -> (OperationError, Option<Data>) {
    use ai_toolbox_management::transport::{ContextClientError, ContextErrorCode};
    match error {
        ContextClientError::Owner(error) => (context_owner_error(error), None),
        ContextClientError::OutcomeUnknown => (OperationError {
            code: ErrorCode::OutcomeUnknown,
            stage: Stage::Context,
            retryable: false,
            message: "The set outcome is unknown. Do not retry automatically; explicitly query request-status with the same request ID. Disconnect is not rollback.",
        }, None),
        ContextClientError::Rejected(error) => {
            let unsupported = error.code == ContextErrorCode::Unsupported;
            (OperationError {
                code: if unsupported { ErrorCode::NotSupported } else { ErrorCode::ContextRejected },
                stage: Stage::Context,
                retryable: false,
                message: if unsupported {
                    "This owner does not enable the requested context operation; local client support is not owner write readiness."
                } else {
                    "The owner rejected the context operation. Review the whitelisted service error and any partial outcome; do not retry automatically."
                },
            }, Some(Data::ContextRejection(error)))
        }
    }
}

#[cfg(target_os = "linux")]
fn context_outcome(outcome: ai_toolbox_management::transport::ContextOutcome) -> ContextDispatch {
    use ai_toolbox_management::transport::ContextPhase;
    if outcome.phase == ContextPhase::Verified {
        Ok(Data::ContextOutcome(outcome))
    } else {
        Err((OperationError {
            code: ErrorCode::ContextPartial,
            stage: Stage::Context,
            retryable: false,
            message: "The recorded context operation is not verified. Partial effects may exist; do not automatically retry, resume, or roll back.",
        }, Some(Data::ContextOutcome(outcome))))
    }
}

impl Envelope {
    pub fn exit_code(&self) -> u8 {
        match &self.result {
            Ok(_) => 0,
            Err(error) => error.code.exit_code(),
        }
    }

    pub fn to_json(&self) -> String {
        let operation = self
            .operation
            .map(|op| json_string(op.id()))
            .unwrap_or_else(|| "null".into());
        let (data, error) = match &self.result {
            Ok(data) => (data.to_json(), "null".into()),
            Err(error) => (
                self.partial
                    .as_ref()
                    .map(Data::to_json)
                    .unwrap_or_else(|| "null".into()),
                format!(
                    "{{\"code\":{},\"stage\":{},\"retryable\":{},\"message\":{}}}",
                    json_string(error.code.as_str()),
                    json_string(error.stage.as_str()),
                    error.retryable,
                    json_string(error.message)
                ),
            ),
        };
        // Discovery/status retain null. A validated context ID identifies the
        // requested operation but is not a claim that the owner committed it.
        let request_id = self
            .request_id
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".into());
        let warnings = if matches!(
            self.operation,
            Some(
                Operation::ContextInspect | Operation::ContextSet | Operation::ContextRequestStatus
            )
        ) {
            "[\"Context inspection describes merged master overrides, not model defaults or active client state. Recorded verification is historical, not proof of current runtime state or client reload.\"]"
        } else {
            "[]"
        };
        format!("{{\"schema_version\":{SCHEMA_VERSION},\"operation\":{operation},\"request_id\":{request_id},\"ok\":{},\"data\":{data},\"warnings\":{warnings},\"error\":{error}}}", self.result.is_ok())
    }

    pub fn to_text(&self) -> String {
        match &self.result {
            Ok(Data::Help) => HELP.into(),
            Ok(Data::Version) => format!(
                "ai-toolbox-cli {VERSION} (automation foundation; schema {SCHEMA_VERSION})\n"
            ),
            Ok(Data::Capabilities) => {
                let mut result = String::from(
                    "Automation foundation. Business coverage: partial (backup path normalization and static gateway capability query). MCP: stdio 2025-11-25.\n",
                );
                for operation in Operation::ALL {
                    let _ = writeln!(
                        result,
                        "{}: {} ({})",
                        operation.id(),
                        operation.command(),
                        if operation.supported() {
                            "supported"
                        } else {
                            "NOT_SUPPORTED"
                        }
                    );
                }
                result.push_str("daemon.status is Linux-only, partial profile-lease inspection; full daemon lifecycle status is not implemented.\n");
                let _ = writeln!(result, "Context read client supported: {}. ContextSet is reserved and sends no request. Owner readiness: not checked. Owner write enablement: not established. Local syntax does not enable ContextSet on an owner.", cfg!(target_os = "linux"));
                result
            }
            Ok(Data::BackupPath(path)) => format!("{path}\n"),
            Ok(Data::GatewayCliKeys(keys)) => format!("{}\n", keys.join("\n")),
            Ok(Data::OwnerStatus(status)) => format!(
                "profile_lease_held: {}\nThis does not establish database readiness or complete writer admission.\n",
                status.profile_lease_held
            ),
            #[cfg(target_os = "linux")]
            Ok(data @ (Data::ContextInspection(_) | Data::ContextOutcome(_) | Data::ContextRejection(_))) => data.context_text(),
            Err(error) => {
                let mut result = format!("{} [{}]: {}\n", error.code.as_str(), error.stage.as_str(), error.message);
                if let Some(id) = &self.request_id { let _ = writeln!(result, "request_id: {id}"); }
                #[cfg(target_os = "linux")]
                if let Some(data) = &self.partial { result.push_str(&data.context_text()); }
                result
            },
        }
    }
}

impl Data {
    fn to_json(&self) -> String {
        match self {
            Self::Help => format!("{{\"text\":{}}}", json_string(HELP)),
            Self::Version => format!(
                "{{\"name\":\"ai-toolbox-cli\",\"version\":{},\"stage\":\"foundation\"}}",
                json_string(VERSION)
            ),
            Self::Capabilities => {
                let operations = Operation::ALL.map(|operation| {
                    let context = matches!(operation, Operation::ContextInspect | Operation::ContextSet | Operation::ContextRequestStatus);
                    let input = if context { "explicit_owner_paths_and_closed_context_arguments" }
                        else if operation == Operation::DaemonStatus { "explicit_absolute_profile_and_private_endpoint_dir" }
                        else if operation == Operation::BackupNormalizePath { "explicit_path_max_32768_bytes" } else { "none" };
                    let extra = if context {
                        format!(",\"local_client_supported\":{},\"owner_readiness\":\"not_checked\",\"owner_write_enablement\":\"not_established\",\"platforms\":[\"linux\"]", cfg!(target_os = "linux") && operation != Operation::ContextSet)
                    } else if operation == Operation::DaemonStatus {
                        ",\"status_scope\":\"profile_lease_only\",\"daemon_lifecycle_complete\":false,\"platforms\":[\"linux\"]".into()
                    } else { String::new() };
                    format!("{{\"operation_id\":{},\"command\":{},\"supported\":{},\"side_effects\":{},\"input\":{}{extra}}}",
                        json_string(operation.id()), json_string(operation.command()), operation.supported(),
                        false, json_string(input))
                }).join(",");
                format!("{{\"stage\":\"foundation\",\"business_coverage\":\"partial\",\"mcp_supported\":true,\"mcp_protocol_version\":\"2025-11-25\",\"mcp_tools\":[\"backup_normalize_path\",\"gateway_supported_cli_keys\"],\"operations\":[{operations}]}}")
            }
            Self::GatewayCliKeys(keys) => serde_json::json!({"cli_keys": keys}).to_string(),
            Self::BackupPath(path) => format!("{{\"path\":{}}}", json_string(path)),
            Self::OwnerStatus(status) => format!("{{\"profile_lease_held\":{}}}", status.profile_lease_held),
            #[cfg(target_os = "linux")]
            Self::ContextInspection(value) => format!("{{\"scope\":\"merged_master_overrides\",\"provider_id\":{},\"revision\":{},\"model_context_window\":{},\"model_auto_compact_token_limit\":{}}}",
                json_string(&value.provider_id), json_string(&value.revision), context_override_json(&value.model_context_window), context_override_json(&value.model_auto_compact_token_limit)),
            #[cfg(target_os = "linux")]
            Self::ContextOutcome(value) => context_outcome_json(value),
            #[cfg(target_os = "linux")]
            Self::ContextRejection(value) => format!("{{\"service_error\":{},\"partial\":{}}}", json_string(context_error_name(value.code)), value.partial.as_ref().map(context_outcome_json).unwrap_or_else(|| "null".into())),
        }
    }
}

#[cfg(target_os = "linux")]
fn optional_number(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".into())
}

#[cfg(target_os = "linux")]
fn context_override_json(value: &ai_toolbox_management::transport::ContextOverride) -> String {
    use ai_toolbox_management::transport::ContextSource;
    let source = match value.source {
        ContextSource::Common => "common",
        ContextSource::Provider => "provider",
        ContextSource::Unspecified => "unspecified",
    };
    format!(
        "{{\"effective\":{},\"source\":{}}}",
        optional_number(value.effective),
        json_string(source)
    )
}

#[cfg(target_os = "linux")]
fn context_phase_name(phase: ai_toolbox_management::transport::ContextPhase) -> &'static str {
    use ai_toolbox_management::transport::ContextPhase;
    match phase {
        ContextPhase::SavedOnly => "saved_only",
        ContextPhase::ProjectionStarted => "projection_started",
        ContextPhase::ProjectedUnverified => "projected_unverified",
        ContextPhase::Verified => "verified",
        ContextPhase::ProjectionUncertain => "projection_uncertain",
    }
}

#[cfg(target_os = "linux")]
fn context_outcome_json(value: &ai_toolbox_management::transport::ContextOutcome) -> String {
    let failure = value
        .failure
        .map(|failure| json_string(context_failure_name(failure)))
        .unwrap_or_else(|| "null".into());
    format!("{{\"request_id\":{},\"provider_id\":{},\"phase\":{},\"failure\":{failure},\"replay\":{},\"model_context_window\":{},\"model_auto_compact_token_limit\":{},\"verification_scope\":\"config_file_at_operation_time\"}}",
        json_string(&value.request_id), json_string(&value.provider_id), json_string(context_phase_name(value.phase)), value.replay,
        optional_number(value.model_context_window), optional_number(value.model_auto_compact_token_limit))
}

#[cfg(target_os = "linux")]
fn context_failure_name(failure: ai_toolbox_management::transport::ContextFailure) -> &'static str {
    use ai_toolbox_management::transport::ContextFailure;
    match failure {
        ContextFailure::ProjectionPreflightFailed => "projection_preflight_failed",
        ContextFailure::ProjectionOutcomeUnknown => "projection_outcome_unknown",
        ContextFailure::ReadbackFailed => "readback_failed",
        ContextFailure::ReadbackMismatch => "readback_mismatch",
    }
}

#[cfg(target_os = "linux")]
fn context_error_name(code: ai_toolbox_management::transport::ContextErrorCode) -> &'static str {
    use ai_toolbox_management::transport::ContextErrorCode;
    match code {
        ContextErrorCode::Unsupported => "unsupported",
        ContextErrorCode::InvalidRequest => "invalid_request",
        ContextErrorCode::RequestNotFound => "request_not_found",
        ContextErrorCode::Ownership => "ownership",
        ContextErrorCode::Storage => "storage",
        ContextErrorCode::InvalidMaster => "invalid_master",
        ContextErrorCode::ProviderChanged => "provider_changed",
        ContextErrorCode::RevisionConflict => "revision_conflict",
        ContextErrorCode::RuntimeUnsupported => "runtime_unsupported",
        ContextErrorCode::FileUnavailable => "file_unavailable",
        ContextErrorCode::JournalConflict => "journal_conflict",
        ContextErrorCode::JournalInvalid => "journal_invalid",
        ContextErrorCode::PersistenceUncertain => "persistence_uncertain",
        ContextErrorCode::PhasePersistence => "phase_persistence",
        ContextErrorCode::Interrupted => "interrupted",
    }
}

#[cfg(target_os = "linux")]
impl Data {
    fn context_text(&self) -> String {
        match self {
            Self::ContextInspection(value) => format!("Merged master overrides only; not model defaults, live verification, or active client state.\nprovider_id: {}\nrevision: {}\nmodel_context_window: {}\nmodel_auto_compact_token_limit: {}\n",
                value.provider_id, value.revision, context_override_json(&value.model_context_window), context_override_json(&value.model_auto_compact_token_limit)),
            Self::ContextOutcome(value) => format!("request_id: {}\nprovider_id: {}\nphase: {}\nfailure: {}\nreplay: {}\nmodel_context_window: {}\nmodel_auto_compact_token_limit: {}\nVerification is historical; current runtime state and client reload are not verified.\n",
                value.request_id, value.provider_id, context_phase_name(value.phase), value.failure.map(context_failure_name).unwrap_or("none"), value.replay,
                optional_number(value.model_context_window), optional_number(value.model_auto_compact_token_limit)),
            Self::ContextRejection(value) => {
                let mut result = format!("service_error: {}\n", context_error_name(value.code));
                if let Some(outcome) = &value.partial { result.push_str(&Self::ContextOutcome(outcome.clone()).context_text()); }
                result
            }
            _ => String::new(),
        }
    }
}

// Small, output-only encoder. Never accept raw JSON fragments from callers.
// Rust strings are valid Unicode; JSON requires escaping U+0000..U+001F,
// quotes, and backslashes. All other Unicode scalar values are preserved.
fn json_string(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\u{00}'..='\u{1f}' => {
                let _ = write!(result, "\\u{:04x}", character as u32);
            }
            _ => result.push(character),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escapes_every_control_character_and_preserves_unicode() {
        let controls: String = (0..32)
            .map(|value| char::from_u32(value).unwrap())
            .collect();
        let expected = (0..32)
            .map(|value| format!("\\u{value:04x}"))
            .collect::<String>();
        assert_eq!(json_string(&controls), format!("\"{expected}\""));
        assert_eq!(json_string("中文 🦀 \"\\\n"), "\"中文 🦀 \\\"\\\\\\u000a\"");
    }

    #[test]
    fn static_declared_support_matches_dispatch() {
        for command in [
            Command::Help,
            Command::Version,
            Command::Capabilities,
            Command::CodexStatus,
        ] {
            let operation = command.operation();
            let envelope = execute(command);
            assert_eq!(operation.supported(), envelope.result.is_ok());
            assert_eq!(operation.supported(), envelope.exit_code() == 0);
        }
    }
}
