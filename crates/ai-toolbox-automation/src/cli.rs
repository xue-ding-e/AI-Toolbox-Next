//! Pure argument adapter. Even parsing failures do not initialize application state.
use crate::{execute, Command, Envelope, OperationError, OwnerStatusInput};
use std::ffi::OsString;
use std::path::PathBuf;

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
        let mut profile = None;
        let mut endpoint_dir = None;
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
            } else if matches!(args[index], "--profile" | "--endpoint-dir") {
                let destination = if args[index] == "--profile" {
                    &mut profile
                } else {
                    &mut endpoint_dir
                };
                if destination.is_some() {
                    return Err(OperationError::invalid_input());
                }
                index += 1;
                *destination = Some(PathBuf::from(
                    *args.get(index).ok_or_else(OperationError::invalid_input)?,
                ));
            } else {
                command.push(args[index]);
            }
            index += 1;
        }
        // Parsing and absolute-path checks finish before any directory inspection.
        // Discovery does not accept or inspect owner paths, even when supplied.
        let no_owner_paths = profile.is_none() && endpoint_dir.is_none();
        match command.as_slice() {
            ["--help" | "-h"] if no_owner_paths => Ok(Command::Help),
            ["--version" | "-V"] if no_owner_paths => Ok(Command::Version),
            ["capabilities"] if no_owner_paths => Ok(Command::Capabilities),
            ["codex", "status"] if no_owner_paths => Ok(Command::CodexStatus),
            ["daemon", "status"] => OwnerStatusInput::new(
                profile.ok_or_else(OperationError::invalid_input)?,
                endpoint_dir.ok_or_else(OperationError::invalid_input)?,
            ).map(Command::DaemonStatus),
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
