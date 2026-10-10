mod files;
mod json;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use axs::{AxsError, ErrorKind};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "axs",
    version,
    about = "Validate, format, and convert AXS documents",
    disable_help_subcommand = true
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate an AXS file without modifying it.
    Check { file: PathBuf },
    /// Canonically format an AXS file; comments and original ordering are lost.
    Fmt {
        /// Report noncanonical formatting without modifying the file.
        #[arg(long)]
        check: bool,
        file: PathBuf,
    },
    /// Write JSON to stdout from an AXS file.
    ToJson { file: PathBuf },
    /// Write canonical AXS to stdout from an object-root JSON file.
    FromJson { file: PathBuf },
}

struct Failure {
    code: u8,
    message: String,
}

impl Failure {
    fn document(error: AxsError, source: &str, path: &std::path::Path) -> Self {
        Self {
            code: 1,
            message: error.render(source, &path.to_string_lossy()),
        }
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: AxsError::new(ErrorKind::InvalidValue, message).to_string(),
        }
    }
    fn io(context: &str, error: io::Error) -> Self {
        Self {
            code: 3,
            message: format!("I/O error: {context}: {error}"),
        }
    }
}

pub(super) fn main() -> ExitCode {
    let args = match Arguments::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let code = error.exit_code() as u8;
            if error.print().is_err() {
                return ExitCode::from(3);
            }
            return ExitCode::from(code);
        }
    };
    match run(args.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // A closed stderr must not cause a panic or obscure the exit status.
            let _ = writeln!(io::stderr().lock(), "{}", error.message);
            ExitCode::from(error.code)
        }
    }
}

fn run(command: Command) -> Result<(), Failure> {
    let path = match &command {
        Command::Check { file }
        | Command::Fmt { file, .. }
        | Command::ToJson { file }
        | Command::FromJson { file } => file,
    };
    let in_place = matches!(command, Command::Fmt { check: false, .. });
    let loaded = files::read(path, in_place)?;
    if matches!(command, Command::FromJson { .. }) {
        let tree = json::from_json(&loaded.source)
            .map_err(|error| Failure::document(error, &loaded.source, path))?;
        let encoded = axs::stringify(&tree).map_err(|message| Failure { code: 1, message })?;
        return stdout(encoded.as_bytes());
    }
    let tree = axs::parse(&loaded.source)
        .map_err(|error| Failure::document(error, &loaded.source, path))?;
    match command {
        Command::Check { .. } => Ok(()),
        Command::Fmt { check, .. } => {
            let encoded = axs::stringify(&tree).map_err(|message| Failure { code: 1, message })?;
            if encoded == loaded.source {
                return Ok(());
            }
            if check {
                return Err(Failure::invalid("File is not canonically formatted"));
            }
            files::replace(&loaded, encoded.as_bytes())
        }
        Command::ToJson { .. } => {
            let encoded = json::to_json(tree)
                .map_err(|error| Failure::document(error, &loaded.source, path))?;
            stdout(&encoded)
        }
        Command::FromJson { .. } => Err(Failure::invalid("Unexpected conversion command")),
    }
}

fn stdout(bytes: &[u8]) -> Result<(), Failure> {
    let mut output = io::stdout().lock();
    output
        .write_all(bytes)
        .and_then(|()| output.flush())
        .map_err(|error| Failure::io("writing stdout", error))
}
