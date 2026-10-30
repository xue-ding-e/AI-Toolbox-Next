//! Pure argument adapter. Even parsing failures do not initialize application state.
use crate::{BackupPathInput, execute, Command, ContextRequestStatusInput, ContextSetInput, Envelope, OperationError, OwnerStatusInput};
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
    let mut format = if json_requested { Format::Json } else { Format::Text };
    let parsed = parse(args, &mut format);
    let envelope = match parsed {
        Ok(operation) => execute(operation),
        Err(error) => {
            if json_requested { format = Format::Json; }
            Envelope {
                operation: None,
                request_id: None,
                partial: None,
                result: Err(error),
            }
        }
    };
    Response { format, envelope }
}

// Structural parsing is platform independent. Linux then uses the transport's
// sole typed validators before inspecting any path. Other platforms fail with
// NOT_SUPPORTED instead of loading a transport or duplicating business rules.
fn parse(args: &[OsString], format: &mut Format) -> Result<Command, OperationError> {
    let args = args.iter().map(|arg| arg.to_str()).collect::<Option<Vec<_>>>()
        .ok_or_else(OperationError::invalid_input)?;
    let mut command = Vec::new();
    let mut options = std::collections::BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index];
        if matches!(argument, "--format" | "--profile" | "--endpoint-dir" | "--request-id"
            | "--expected-provider-id" | "--expected-revision" | "--target"
            | "--model-context-window" | "--model-auto-compact-token-limit" | "--path") {
            index += 1;
            let value = *args.get(index).ok_or_else(OperationError::invalid_input)?;
            if options.insert(argument, value).is_some() {
                return Err(OperationError::invalid_input());
            }
        } else {
            command.push(argument);
        }
        index += 1;
    }
    if let Some(value) = options.remove("--format") {
        *format = match value {
            "text" => Format::Text,
            "json" => Format::Json,
            _ => return Err(OperationError::invalid_input()),
        };
    }
    // Parsing and absolute-path checks finish before any directory inspection.
    // Discovery does not accept or inspect owner paths, even when supplied.
    match command.as_slice() {
        ["--help" | "-h"] if options.is_empty() => return Ok(Command::Help),
        ["--version" | "-V"] if options.is_empty() => return Ok(Command::Version),
        ["capabilities"] if options.is_empty() => return Ok(Command::Capabilities),
        ["codex", "status"] if options.is_empty() => return Ok(Command::CodexStatus),
        ["backup", "normalize-path"] => {
            let path = required(&mut options, "--path")?;
            if !options.is_empty() { return Err(OperationError::invalid_input()); }
            return Ok(Command::BackupNormalizePath(BackupPathInput::new(path.to_owned())?));
        }
        ["daemon", "status"] | ["codex", "context", "inspect" | "set" | "request-status"] => {}
        _ => return Err(OperationError::invalid_input()),
    }
    let owner = OwnerStatusInput::new(
        PathBuf::from(required(&mut options, "--profile")?),
        PathBuf::from(required(&mut options, "--endpoint-dir")?),
    )?;
    let parsed = match command.as_slice() {
        ["daemon", "status"] => Command::DaemonStatus(owner),
        ["codex", "context", "inspect"] => Command::ContextInspect(owner),
        ["codex", "context", "request-status"] => {
            let request_id = required(&mut options, "--request-id")?.to_owned();
            Command::ContextRequestStatus(ContextRequestStatusInput::new(owner, request_id)?)
        }
        ["codex", "context", "set"] => {
            let request_id = required(&mut options, "--request-id")?.to_owned();
            let expected_provider_id = required(&mut options, "--expected-provider-id")?.to_owned();
            let expected_revision = required(&mut options, "--expected-revision")?.to_owned();
            let target = required(&mut options, "--target")?;
            if !matches!(target, "common" | "provider") {
                return Err(OperationError::invalid_input());
            }
            let model_context_window = number(options.remove("--model-context-window"))?;
            let model_auto_compact_token_limit = number(options.remove("--model-auto-compact-token-limit"))?;
            if model_context_window.is_none() && model_auto_compact_token_limit.is_none() {
                return Err(OperationError::invalid_input());
            }
            #[cfg(target_os = "linux")]
            {
                use ai_toolbox_management::transport::{ContextPatch, ContextSetRequest, ContextTarget};
                Command::ContextSet(ContextSetInput::new(owner, ContextSetRequest {
                    schema_version: 1,
                    request_id,
                    expected_provider_id,
                    expected_revision,
                    target: if target == "common" { ContextTarget::Common } else { ContextTarget::Provider },
                    patch: ContextPatch { model_context_window, model_auto_compact_token_limit },
                })?)
            }
            #[cfg(not(target_os = "linux"))]
            {
                // No semantic validator or owner transport is linked here.
                let _ = (request_id, expected_provider_id, expected_revision, target);
                Command::ContextSet(ContextSetInput { owner })
            }
        }
        _ => return Err(OperationError::invalid_input()),
    };
    if !options.is_empty() { return Err(OperationError::invalid_input()); }
    Ok(parsed)
}

fn required<'a>(options: &mut std::collections::BTreeMap<&str, &'a str>, key: &str) -> Result<&'a str, OperationError> {
    options.remove(key).ok_or_else(OperationError::invalid_input)
}

fn number(value: Option<&str>) -> Result<Option<u64>, OperationError> {
    value.map(|value| {
        // Decimal syntax only; range and paired-value semantics belong to the
        // transport validator, not a second CLI-specific business validator.
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(OperationError::invalid_input());
        }
        value.parse().map_err(|_| OperationError::invalid_input())
    }).transpose()
}
