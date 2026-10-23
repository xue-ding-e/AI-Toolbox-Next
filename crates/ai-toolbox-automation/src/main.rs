// Keep the default console subsystem on Windows, including release builds.
use ai_toolbox_automation::cli::{run, Format};
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let response = run(&std::env::args_os().skip(1).collect::<Vec<_>>());
    let output = match response.format {
        Format::Json => format!("{}\n", response.envelope.to_json()),
        Format::Text => response.envelope.to_text(),
    };
    let written = if response.format == Format::Text && response.envelope.result.is_err() {
        io::stderr().lock().write_all(output.as_bytes())
    } else {
        io::stdout().lock().write_all(output.as_bytes())
    };
    if written.is_err() {
        return ExitCode::from(74);
    }
    ExitCode::from(response.envelope.exit_code())
}
