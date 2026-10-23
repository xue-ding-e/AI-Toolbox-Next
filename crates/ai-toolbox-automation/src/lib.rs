//! Headless contract foundation, not a business service implementation.
//!
//! Schema version 1 is independent of desktop/package versions. Breaking wire
//! changes require a new schema version; consumers must reject unknown versions.
//! Discovery never resolves paths or initializes application state. The explicit
//! Linux owner-status client only inspects existing paths and connects to an owner;
//! it never creates paths/leases, opens a database, or starts a GUI/service owner.

use std::fmt::Write;
use std::path::PathBuf;

pub mod cli;

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
Exit codes: 0 success, 2 INVALID_INPUT, 3 NOT_SUPPORTED, 4 invalid owner paths,\n\
5 OWNER_UNAVAILABLE, 6 owner protocol/identity rejection, 7 OWNER_TIMEOUT,\n\
70 owner client failure, 74 output I/O failure.\n\
JSON results go to stdout, including errors; text errors go to stderr.\n\
Status does not prove DB readiness or complete writer admission. No daemon is started.\n\
PID, start time, and full daemon lifecycle status are not implemented.\n\
No business services, MCP server, or P0 workflow are implemented.\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Help,
    Version,
    Capabilities,
    CodexStatus,
    DaemonStatus,
}

impl Operation {
    pub const ALL: [Self; 5] = [
        Self::Help,
        Self::Version,
        Self::Capabilities,
        Self::CodexStatus,
        Self::DaemonStatus,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Help => "automation.help",
            Self::Version => "automation.version",
            Self::Capabilities => "automation.capabilities",
            Self::CodexStatus => "codex.status",
            Self::DaemonStatus => "daemon.status",
        }
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Help => "--help",
            Self::Version => "--version",
            Self::Capabilities => "capabilities",
            Self::CodexStatus => "codex status",
            Self::DaemonStatus => "daemon status",
        }
    }

    pub fn supported(self) -> bool {
        match self {
            Self::CodexStatus => false,
            Self::DaemonStatus => cfg!(target_os = "linux"),
            _ => true,
        }
    }
}

/// Closed command input. Paths cannot be constructed without pure validation.
/// There are no environment defaults, arbitrary invokes, or business-write DTOs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Capabilities,
    CodexStatus,
    DaemonStatus(OwnerStatusInput),
}

impl Command {
    fn operation(&self) -> Operation {
        match self {
            Self::Help => Operation::Help,
            Self::Version => Operation::Version,
            Self::Capabilities => Operation::Capabilities,
            Self::CodexStatus => Operation::CodexStatus,
            Self::DaemonStatus(_) => Operation::DaemonStatus,
        }
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
        Ok(Self { profile, endpoint_dir })
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
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Parse,
    Dispatch,
    Inspect,
    Connect,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Dispatch => "dispatch",
            Self::Inspect => "inspect",
            Self::Connect => "connect",
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
    OwnerStatus(OwnerStatusData),
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
    pub result: Result<Data, OperationError>,
}

/// Closed, typed dispatch. There is intentionally no string-based invoke path.
/// Future business operations must use the existing extracted services, not
/// parallel SQL/config mergers or a mocked Tauri AppHandle.
pub fn execute(command: Command) -> Envelope {
    let operation = command.operation();
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
        Command::DaemonStatus(input) => owner_status(input).map(Data::OwnerStatus),
    };
    Envelope {
        operation: Some(operation),
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
    use ai_toolbox_management::owner::{ProfileIdentity, RuntimeDirectory};
    use ai_toolbox_management::transport::OwnerClient;

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
        Ok(OwnerStatusData { profile_lease_held: status.profile_lease_held })
    })
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
        Error::OwnerUnavailable => (ErrorCode::OwnerUnavailable, true, "The existing owner endpoint is unavailable; no owner was started."),
        Error::ProtocolMismatch | Error::InvalidMessage | Error::FrameTooLarge => (ErrorCode::OwnerProtocolRejected, false, "The owner protocol response was incompatible or invalid."),
        Error::PeerRejected | Error::StaleOwner | Error::IdentityChanged | Error::EndpointConflict
            | Error::InvalidProfile | Error::UnsafeDirectory => (ErrorCode::OwnerIdentityRejected, false, "The owner, profile, or endpoint identity could not be verified."),
        Error::TimedOut => (ErrorCode::OwnerTimeout, true, "The local owner-status request timed out."),
        // Never format raw I/O or protocol errors into public diagnostics.
        _ => return client_failure(),
    };
    OperationError { code, stage: Stage::Connect, retryable, message }
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
                "null".into(),
                format!(
                    "{{\"code\":{},\"stage\":{},\"retryable\":{},\"message\":{}}}",
                    json_string(error.code.as_str()),
                    json_string(error.stage.as_str()),
                    error.retryable,
                    json_string(error.message)
                ),
            ),
        };
        // Neither discovery nor the one-connection status client has a persisted
        // request/job identity. Transport correlation is not public idempotency.
        format!("{{\"schema_version\":{SCHEMA_VERSION},\"operation\":{operation},\"request_id\":null,\"ok\":{},\"data\":{data},\"warnings\":[],\"error\":{error}}}", self.result.is_ok())
    }

    pub fn to_text(&self) -> String {
        match &self.result {
            Ok(Data::Help) => HELP.into(),
            Ok(Data::Version) => format!(
                "ai-toolbox-cli {VERSION} (automation foundation; schema {SCHEMA_VERSION})\n"
            ),
            Ok(Data::Capabilities) => {
                let mut result = String::from(
                    "Automation foundation only. Business coverage: none. MCP: not implemented.\n",
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
                result
            }
            Ok(Data::OwnerStatus(status)) => format!(
                "profile_lease_held: {}\nThis does not establish database readiness or complete writer admission.\n",
                status.profile_lease_held
            ),
            Err(error) => format!(
                "{} [{}]: {}\n",
                error.code.as_str(),
                error.stage.as_str(),
                error.message
            ),
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
                let operations = Operation::ALL.map(|operation| format!(
                    "{{\"operation_id\":{},\"command\":{},\"supported\":{},\"side_effects\":false,\"input\":{}{}}}",
                    json_string(operation.id()), json_string(operation.command()), operation.supported(),
                    json_string(if operation == Operation::DaemonStatus { "explicit_absolute_profile_and_private_endpoint_dir" } else { "none" }),
                    if operation == Operation::DaemonStatus { ",\"status_scope\":\"profile_lease_only\",\"daemon_lifecycle_complete\":false,\"platforms\":[\"linux\"]" } else { "" }
                )).join(",");
                format!("{{\"stage\":\"foundation\",\"business_coverage\":\"none\",\"mcp_supported\":false,\"operations\":[{operations}]}}")
            }
            Self::OwnerStatus(status) => format!("{{\"profile_lease_held\":{}}}", status.profile_lease_held),
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
        for command in [Command::Help, Command::Version, Command::Capabilities, Command::CodexStatus] {
            let operation = command.operation();
            let envelope = execute(command);
            assert_eq!(operation.supported(), envelope.result.is_ok());
            assert_eq!(operation.supported(), envelope.exit_code() == 0);
        }
    }
}
