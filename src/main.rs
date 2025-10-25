//! Defines the CLI. Only this file should depend on clap.

mod cli_error;
mod cli_subcommands;
mod cli_value;
#[cfg(test)]
mod main_test;
mod msgpack;
mod sqlite_escape;
mod utf8_extractor;
mod util;

use std::io::Write;
use std::path::PathBuf;

/// This is the SQLite bindings for the VSCode extension "SQLite3 Editor" (https://marketplace.visualstudio.com/items?itemName=yy0931.vscode-sqlite3-editor). The source code is available at: https://github.com/yy0931/sqlite3-editor/tree/rust-backend
#[derive(clap::Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, ts_rs::TS)]
#[ts(export, rename_all = "kebab-case")]
enum Commands {
    Version {},
    FunctionList {},
    Import {
        /// Path to the database file
        #[arg(long, required = true)]
        database_filepath: String,

        #[arg(long, required = true)]
        format: ImportingFileFormat,
        #[arg(long, required = true)]
        table_name: String,
        #[arg(long, default_value = ",")]
        csv_delimiter: String,
        #[arg(long)]
        input_file: Option<String>,
    },
    Export {
        /// Path to the database file
        #[arg(long, required = true)]
        database_filepath: String,

        #[arg(long, required = true)]
        format: ExportingFileFormat,
        #[arg(long)]
        query: String,
        #[arg(long)]
        output_file: Option<String>,

        #[arg(long)]
        csv_options: Option<String>,
        #[arg(long)]
        xlsx_options: Option<String>,
    },
    Server {
        /// Path to the database file
        #[arg(long, required = true)]
        database_filepath: String,

        /// Path to the file containing the request body
        #[arg(long, required = true)]
        request_body_filepath: PathBuf,

        /// Path where the response body should be written
        #[arg(long, required = true)]
        response_body_filepath: PathBuf,
    },
    CopyFile {
        #[arg(long, required = true)]
        src: PathBuf,

        #[arg(long, required = true)]
        dst: PathBuf,
    },
}

/// The values allowed for the `--format` option of the `import` subcommand.
#[derive(Clone, Debug, Eq, PartialEq, clap::ValueEnum, ts_rs::TS)]
#[ts(export)]
pub enum ImportingFileFormat {
    #[clap(name = "csv")]
    #[ts(rename = "csv")]
    Csv,
    #[clap(name = "tsv")]
    #[ts(rename = "tsv")]
    Tsv,
    #[clap(name = "json")]
    #[ts(rename = "json")]
    Json,
}

/// The values allowed for the `--format` option of the `export` subcommand.
#[derive(Clone, Debug, Eq, PartialEq, clap::ValueEnum, ts_rs::TS)]
#[ts(export)]
enum ExportingFileFormat {
    #[clap(name = "csv")]
    #[ts(rename = "csv")]
    Csv,
    #[clap(name = "json")]
    #[ts(rename = "json")]
    Json,
    #[clap(name = "xlsx")]
    #[ts(rename = "xlsx")]
    Xlsx,
}

fn main() {
    use clap::Parser;

    // Parse the command line arguments
    let code = cli(
        Args::parse(),
        || std::io::stdin().lock(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    if code != 0 {
        std::process::exit(code);
    }
}

fn cli<F, I, O, E>(args: Args, stdin: F, stdout: &mut O, stderr: &mut E) -> i32
where
    F: FnOnce() -> I + std::marker::Send + 'static,
    I: cli_subcommands::server::ReadCommand,
    O: Write,
    E: Write,
{
    match args.command {
        Commands::Version {} => {
            cli_subcommands::version::run(stdout);
            0
        }
        Commands::FunctionList {} => {
            cli_subcommands::function_list::run(stdout);
            0
        }
        Commands::Server {
            database_filepath,
            request_body_filepath,
            response_body_filepath,
        } => cli_subcommands::server::run(
            stdin,
            stdout,
            stderr,
            database_filepath,
            request_body_filepath,
            response_body_filepath,
        ),
        Commands::Export {
            database_filepath,
            format,
            query,
            output_file,
            csv_options,
            xlsx_options,
        } => cli_subcommands::export::run(
            stdout,
            stderr,
            database_filepath,
            format,
            query,
            output_file,
            csv_options,
            xlsx_options,
        ),
        Commands::Import {
            database_filepath,
            format,
            table_name,
            csv_delimiter,
            input_file,
        } => cli_subcommands::import::run(stderr, database_filepath, format, table_name, csv_delimiter, input_file),
        Commands::CopyFile { src, dst } => cli_subcommands::copy_file::run(stderr, src, dst),
    }
}
