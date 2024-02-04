use rmp_serde::encode::write_named;
use std::{
    fs::File,
    io::{BufRead, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};
mod completion;
#[cfg(test)]
mod completion_test;
mod keywords;
#[cfg(test)]
mod keywords_test;
#[cfg(test)]
mod main_test;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
mod column_origin;
mod export;
mod import;
use crate::{request_type::Request, sqlite3_driver::read_msgpack_into_json, tokenize::ZeroIndexedLocation};
mod cache;
mod check_syntax;
#[cfg(test)]
mod check_syntax_test;
mod code_lens;
#[cfg(test)]
mod code_lens_test;
#[cfg(test)]
mod column_origin_test;
mod error;
#[cfg(test)]
mod export_test;
mod find;
#[cfg(test)]
mod import_test;
mod literal;
#[cfg(test)]
mod literal_test;
mod online_backup;
#[cfg(test)]
mod online_backup_test;
mod parse_cte;
#[cfg(test)]
mod parse_cte_test;
mod request_type;
#[cfg(test)]
mod request_type_test;
mod semantic_highlight;
#[cfg(test)]
mod semantic_highlight_test;
mod split_statements;
#[cfg(test)]
mod split_statements_test;
mod sqlite3_driver;
#[cfg(test)]
mod sqlite3_driver_test;
mod tokenize;
#[cfg(test)]
mod tokenize_test;
#[cfg(test)]
mod type_test;
mod util;

#[cfg(all(feature = "sqlite", feature = "sqlcipher"))]
compile_error!("Cannot enable both 'sqlite' and 'sqlcipher' features.");

#[cfg(not(any(feature = "sqlite", feature = "sqlcipher")))]
compile_error!("Must use `--features sqlite` or `--features sqlcipher` command line option.");

/// SQLite bindings for https://marketplace.visualstudio.com/items?itemName=yy0931.vscode-sqlite3-editor
///
/// Source: https://github.com/yy0931/sqlite3-editor/tree/rust-backend
#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum ImportFormat {
    #[clap(name = "csv")]
    CSV,
    #[clap(name = "tsv")]
    TSV,
    #[clap(name = "json")]
    JSON,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    #[clap(name = "csv")]
    CSV,
    #[clap(name = "tsv")]
    TSV,
    #[clap(name = "json")]
    JSON,
    #[clap(name = "xlsx")]
    XLSX,
}

#[derive(Subcommand)]
enum Commands {
    Version {},
    FunctionList {},
    Import {
        /// Path to the database file
        #[arg(long, required = true)]
        database_filepath: String,
        /// Optional SQL Cipher key for encrypted databases
        #[arg(long)]
        sql_cipher_key: Option<String>,

        #[arg(long, required = true)]
        format: ImportFormat,
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
        /// Optional SQL Cipher key for encrypted databases
        #[arg(long)]
        sql_cipher_key: Option<String>,

        #[arg(long, required = true)]
        format: ExportFormat,
        #[arg(long)]
        query: String,
        #[arg(long, default_value = ",")]
        csv_delimiter: String,
        #[arg(long)]
        output_file: Option<String>,
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

        /// Optional SQL Cipher key for encrypted databases
        #[arg(long)]
        sql_cipher_key: Option<String>,
    },
}

/// Structure representing a database query
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(from = "(String,)")]
struct Query {
    query: String,
}

impl From<(String,)> for Query {
    fn from(value: (String,)) -> Self {
        Self { query: value.0 }
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(from = "(String, i64, i64)")]
struct CompletionQuery {
    sql: String,
    line: i64,
    column: i64,
}

impl From<(String, i64, i64)> for CompletionQuery {
    fn from(value: (String, i64, i64)) -> Self {
        Self {
            sql: value.0,
            line: value.1,
            column: value.2,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ServerCommand {
    Interrupt,
    Close,
    TryReconnect,
    Handle,
    SemanticHighlight,
    CodeLens,
    CheckSyntax,
    Completion,
}

impl FromStr for ServerCommand {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim() {
            "interrupt" => Self::Interrupt,
            "close" => Self::Close,
            "try_reconnect" => Self::TryReconnect,
            "handle" => Self::Handle,
            "semantic_highlight" => Self::SemanticHighlight,
            "code_lens" => Self::CodeLens,
            "check_syntax" => Self::CheckSyntax,
            "completion" => Self::Completion,
            _ => return Err(()),
        })
    }
}

fn cli<F, I, O, E>(args: Args, stdin: F, mut stdout: &mut O, mut stderr: &mut E) -> i32
where
    F: Fn() -> I + std::marker::Send + 'static,
    I: Read + BufRead,
    O: Write,
    E: Write,
{
    match args.command {
        Commands::Version {} => {
            // health check
            let con = rusqlite::Connection::open_in_memory().unwrap();
            con.execute("CREATE TABLE t(v)", ()).unwrap();
            con.execute("INSERT INTO t VALUES (?)", ["ok"]).unwrap();
            assert_eq!(
                con.query_row("SELECT v FROM t", [], |row| row.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
            writeln!(&mut stdout, "sqlite3-editor {}", env!("CARGO_PKG_VERSION")).expect("writeln! failed.");
            writeln!(
                &mut stdout,
                "{} {}",
                if sqlite3_driver::is_sqlcipher(&con) {
                    "SQLCipher"
                } else {
                    "SQLite"
                },
                rusqlite::version()
            )
            .expect("writeln! failed.");

            let conn = rusqlite::Connection::open_in_memory().unwrap();

            writeln!(
                &mut stdout,
                "\nCompile options:\n{}",
                conn.prepare("PRAGMA compile_options")
                    .unwrap()
                    .query_map((), |row| row.get::<_, String>(0))
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
                    .into_iter()
                    .map(|line| "- ".to_owned() + &line)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
            .expect("writeln! failed.");
        }
        Commands::FunctionList {} => {
            let mut functions = rusqlite::Connection::open_in_memory()
                .unwrap()
                .prepare("PRAGMA function_list")
                .unwrap()
                .query_map((), |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            functions.sort();
            writeln!(&mut stdout, "{}", serde_json::to_string(&functions).unwrap()).expect("writeln! failed.");
        }
        Commands::Server {
            database_filepath,
            request_body_filepath,
            sql_cipher_key,
            response_body_filepath,
        } => {
            const READ_ONLY: bool = false;

            // Create a server
            let mut db = match sqlite3_driver::SQLite3Driver::connect(&database_filepath, READ_ONLY, &sql_cipher_key) {
                Ok(db) => db,
                Err(err) => {
                    writeln!(&mut stderr, "{err}").expect("writeln! failed.");
                    return 1;
                }
            };

            let (command_sender, command_receiver) = std::sync::mpsc::channel::<ServerCommand>();
            let interrupt_handle = Arc::new(Mutex::new(db.get_interrupt_handle()));
            let _thread = {
                let interrupt_handle = Arc::clone(&interrupt_handle);
                std::thread::spawn(move || {
                    let mut stdin = stdin();
                    loop {
                        let mut command_str = String::new();
                        match stdin.read_line(&mut command_str) {
                            Err(_) => return,
                            Ok(0) => return,
                            _ => {}
                        }
                        match command_str.parse::<ServerCommand>() {
                            Ok(ServerCommand::Interrupt) => {
                                interrupt_handle.lock().unwrap().interrupt();
                            }
                            Ok(command) => {
                                if command_sender.send(command).is_err() {
                                    return;
                                }
                            }
                            _ => {}
                        }
                    }
                })
            };

            // Start the main loop
            loop {
                let Ok(command) = command_receiver.recv() else {
                    return 0;
                };

                // Terminate the loop before opening the files
                if command == ServerCommand::Close {
                    return 0;
                }

                // Open request and response files
                let mut r = File::open(&request_body_filepath).unwrap();
                let mut w = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&response_body_filepath)
                    .unwrap();

                fn finish<T: Write, U: Write>(mut stdout: &mut T, w: &mut U, code: error::ErrorCode) {
                    w.write_all(b"END").expect("Failed to write the result.");
                    w.flush().expect("Failed to flush the writer.");
                    writeln!(&mut stdout, "{code:?}").expect("writeln! failed.");
                    stdout.flush().unwrap();
                }

                // Handle the different commands
                match command {
                    ServerCommand::TryReconnect => {
                        match sqlite3_driver::SQLite3Driver::connect(&database_filepath, READ_ONLY, &sql_cipher_key) {
                            Ok(new_db) => {
                                db = new_db;

                                *interrupt_handle.lock().unwrap() = db.get_interrupt_handle();
                                write_named(&mut w, &None::<&i64>).expect("Failed to write the result.");
                                finish(&mut stdout, &mut w, error::ErrorCode::Success);
                            }
                            Err(err) => {
                                w.truncate_all();
                                write!(w, "{err}").expect("Failed to write an error message.");
                                finish(&mut stdout, &mut w, err.code());
                            }
                        }
                    }

                    // Handle the request
                    ServerCommand::Handle => {
                        // Deserialize the request
                        let req = match rmp_serde::from_read::<_, Request>(&mut r) {
                            Ok(req) => req,
                            Err(err) => {
                                let mut content = read_msgpack_into_json(&mut r);
                                if content.len() > 5000 {
                                    content = content[0..5000].to_owned() + "... (omitted)"
                                }
                                write!(
                                    w,
                                    "Failed to parse the request body: {err} (content = {}, len = {})",
                                    content,
                                    r.metadata().unwrap().len()
                                )
                                .expect("Failed to write an error message.");
                                finish(&mut stdout, &mut w, error::ErrorCode::OtherError);
                                continue;
                            }
                        };

                        match db.handle(&mut w, &req.query, &req.params, req.mode, req.options) {
                            Ok(_) => {
                                finish(&mut stdout, &mut w, error::ErrorCode::Success);
                            }
                            Err(err) => {
                                w.truncate_all();
                                write!(w, "{err}").expect("Failed to write an error message.");
                                finish(&mut stdout, &mut w, err.code());
                            }
                        }
                    }

                    // Tokenize, get code lenses, or check syntax
                    ServerCommand::SemanticHighlight | ServerCommand::CodeLens | ServerCommand::CheckSyntax => {
                        // Handle the command
                        if let Err(err) = rmp_serde::from_read(r).map(
                            |Query { query }: Query| -> std::result::Result<(), error::Error> {
                                match command {
                                    ServerCommand::SemanticHighlight => {
                                        write_named(&mut w, &semantic_highlight::semantic_highlight(&query))?
                                    }
                                    ServerCommand::CodeLens => write_named(&mut w, &code_lens::code_lens(&query))?,
                                    ServerCommand::CheckSyntax => {
                                        write_named(&mut w, &check_syntax::check_syntax(&query)?)?
                                    }
                                    _ => panic!("Unexpected command {:?}", command),
                                };
                                Ok(())
                            },
                        ) {
                            w.flush().unwrap();
                            w.truncate_all();
                            write!(w, "{err:?}").unwrap();
                            finish(&mut stdout, &mut w, error::ErrorCode::OtherError);
                        } else {
                            w.flush().unwrap();
                            finish(&mut stdout, &mut w, error::ErrorCode::Success);
                        }
                    }

                    // Completion
                    ServerCommand::Completion => {
                        // Handle the command
                        if let Err(err) =
                            rmp_serde::from_read(r).map(|CompletionQuery { sql, line, column }: CompletionQuery| -> std::result::Result<(), error::Error> {
                                write_named(
                                    &mut w,
                                    &completion::complete(
                                        &db,
                                        &sql,
                                        &ZeroIndexedLocation {
                                            line: line.try_into().unwrap(),
                                            column: column.try_into().unwrap(),
                                        },
                                    ),
                                )?;
                                Ok(())
                            })
                        {
                            w.flush().unwrap();
                            w.truncate_all();
                            write!(w, "{err:?}").unwrap();
                            finish(&mut stdout, &mut w, error::ErrorCode::OtherError);
                        } else {
                            w.flush().unwrap();
                            finish(&mut stdout, &mut w, error::ErrorCode::Success);
                        }
                    }

                    // Ignore unrecognized commands
                    _ => {}
                }
            }
        }
        Commands::Export {
            database_filepath,
            sql_cipher_key,
            format,
            query,
            csv_delimiter,
            output_file,
        } => {
            if format == ExportFormat::XLSX {
                let Some(output_file) = output_file else {
                    writeln!(&mut stderr, "`--format xlsx` requires `--output-file <file-name>`.")
                        .expect("writeln! failed.");
                    return 1;
                };
                if let Err(err) = export::export_xlsx(&database_filepath, &sql_cipher_key, &query, &output_file) {
                    writeln!(&mut stderr, "{err}").expect("writeln! failed.");
                    return 1;
                }
            } else {
                let mut writer: Box<dyn Write> = if let Some(output_file) = output_file {
                    let Ok(f) = std::fs::OpenOptions::new()
                        .truncate(true)
                        .create(true)
                        .write(true)
                        .open(output_file)
                    else {
                        return 1;
                    };
                    Box::new(f)
                } else {
                    Box::new(stdout)
                };

                if let Err(err) = match format {
                    ExportFormat::CSV => {
                        export::export_csv(&database_filepath, &sql_cipher_key, &query, &csv_delimiter, &mut writer)
                    }
                    ExportFormat::TSV => {
                        export::export_csv(&database_filepath, &sql_cipher_key, &query, "\t", &mut writer)
                    }
                    ExportFormat::JSON => export::export_json(&database_filepath, &sql_cipher_key, &query, &mut writer),
                    ExportFormat::XLSX => {
                        panic!();
                    }
                } {
                    writeln!(&mut stderr, "{err}").expect("writeln! failed.");
                    return 1;
                }
            }
        }
        Commands::Import {
            database_filepath,
            sql_cipher_key,
            format,
            table_name,
            csv_delimiter,
            input_file,
        } => {
            if let Err(err) = match format {
                ImportFormat::CSV => import::import_csv(
                    &database_filepath,
                    &sql_cipher_key,
                    &table_name,
                    &csv_delimiter,
                    input_file,
                ),
                ImportFormat::TSV => {
                    import::import_csv(&database_filepath, &sql_cipher_key, &table_name, "\t", input_file)
                }
                ImportFormat::JSON => import::import_json(&database_filepath, &sql_cipher_key, &table_name, input_file),
            } {
                writeln!(&mut stderr, "{err}").expect("writeln! failed.");
                return 1;
            }
        }
    }

    0
}

fn main() {
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

pub trait TruncateAll {
    fn truncate_all(&mut self);
}

impl TruncateAll for std::fs::File {
    fn truncate_all(&mut self) {
        self.set_len(0).expect("Failed to truncate the file.");
        self.seek(SeekFrom::Start(0)).expect("Failed to seek the file.");
    }
}

impl TruncateAll for std::io::Cursor<Vec<u8>> {
    fn truncate_all(&mut self) {
        self.get_mut().truncate(0);
        self.seek(SeekFrom::Start(0)).expect("Failed to seek the cursor.");
    }
}
