//! Headless contract foundation, not a business service implementation.
//!
//! Schema version 1 is independent of desktop/package versions. Breaking wire
//! changes require a new schema version; consumers must reject unknown versions.
//! No paths, credentials, database, GUI, network, or service owner are initialized.

use std::fmt::Write;

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
Exit codes: 0 success, 2 INVALID_INPUT, 3 NOT_SUPPORTED, 74 output I/O failure.\n\
JSON results go to stdout, including errors; text errors go to stderr.\n\
No business services, profiles, daemon, MCP server, or P0 workflow are implemented.\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Help,
    Version,
    Capabilities,
    CodexStatus,
}

impl Operation {
    pub const ALL: [Self; 4] = [
        Self::Help,
        Self::Version,
        Self::Capabilities,
        Self::CodexStatus,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Help => "automation.help",
            Self::Version => "automation.version",
            Self::Capabilities => "automation.capabilities",
            Self::CodexStatus => "codex.status",
        }
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Help => "--help",
            Self::Version => "--version",
            Self::Capabilities => "capabilities",
            Self::CodexStatus => "codex status",
        }
    }

    pub fn supported(self) -> bool {
        self != Self::CodexStatus
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidInput,
    NotSupported,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::NotSupported => "NOT_SUPPORTED",
        }
    }

    pub fn exit_code(self) -> u8 {
        match self {
            Self::InvalidInput => 2,
            Self::NotSupported => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Parse,
    Dispatch,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Dispatch => "dispatch",
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
pub fn execute(operation: Operation) -> Envelope {
    let result = match operation {
        Operation::Help => Ok(Data::Help),
        Operation::Version => Ok(Data::Version),
        Operation::Capabilities => Ok(Data::Capabilities),
        Operation::CodexStatus => Err(OperationError {
            code: ErrorCode::NotSupported,
            stage: Stage::Dispatch,
            retryable: false,
            message: "Codex business services are not connected in this foundation.",
        }),
    };
    Envelope {
        operation: Some(operation),
        result,
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
        // No request identifier is invented: these synchronous, local discovery
        // calls have no persisted request/job identity. It is explicitly null.
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
                result
            }
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
                    "{{\"operation_id\":{},\"command\":{},\"supported\":{},\"side_effects\":false,\"input\":\"none\"}}",
                    json_string(operation.id()), json_string(operation.command()), operation.supported()
                )).join(",");
                format!("{{\"stage\":\"foundation\",\"business_coverage\":\"none\",\"mcp_supported\":false,\"operations\":[{operations}]}}")
            }
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
    fn declared_support_matches_dispatch() {
        for operation in Operation::ALL {
            let envelope = execute(operation);
            assert_eq!(operation.supported(), envelope.result.is_ok());
            assert_eq!(operation.supported(), envelope.exit_code() == 0);
        }
    }
}
