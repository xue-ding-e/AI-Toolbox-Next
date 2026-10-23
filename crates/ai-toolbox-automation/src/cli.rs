//! Pure argument adapter. Even parsing failures do not initialize application state.
use crate::{execute, Envelope, Operation, OperationError};
use std::ffi::OsString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
}

pub struct Response {
    pub format: Format,
    pub envelope: Envelope,
}

/// Arguments exclude argv[0]. --format may appear before or after the command.
/// Duplicate options, extra operands, non-Unicode arguments and unknown options
/// fail closed. No supplied argument is copied into an error response.
pub fn run(args: &[OsString]) -> Response {
    // Preserve machine-readable errors even when another argument is malformed.
    let json_requested = args
        .windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json");
    let mut format = if json_requested {
        Format::Json
    } else {
        Format::Text
    };
    let parsed = (|| {
        let args = args
            .iter()
            .map(|arg| arg.to_str())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(OperationError::invalid_input)?;
        let mut command = Vec::new();
        let mut format_seen = false;
        let mut index = 0;
        while index < args.len() {
            if args[index] == "--format" {
                if format_seen {
                    return Err(OperationError::invalid_input());
                }
                format_seen = true;
                index += 1;
                format = match args.get(index) {
                    Some(&"text") => Format::Text,
                    Some(&"json") => Format::Json,
                    _ => return Err(OperationError::invalid_input()),
                };
            } else {
                command.push(args[index]);
            }
            index += 1;
        }
        match command.as_slice() {
            ["--help" | "-h"] => Ok(Operation::Help),
            ["--version" | "-V"] => Ok(Operation::Version),
            ["capabilities"] => Ok(Operation::Capabilities),
            ["codex", "status"] => Ok(Operation::CodexStatus),
            _ => Err(OperationError::invalid_input()),
        }
    })();
    let envelope = match parsed {
        Ok(operation) => execute(operation),
        Err(error) => {
            if json_requested {
                format = Format::Json;
            }
            Envelope {
                operation: None,
                result: Err(error),
            }
        }
    };
    Response { format, envelope }
}
