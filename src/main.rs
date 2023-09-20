use std::{
    fs::File,
    io::{BufRead, Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use rmp_serde::encode::write_named;
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
mod check_syntax;
#[cfg(test)]
mod check_syntax_test;
mod code_lens;
#[cfg(test)]
mod code_lens_test;
#[cfg(test)]
mod column_origin_test;
#[cfg(test)]
mod export_test;
#[cfg(test)]
mod import_test;
mod literal;
#[cfg(test)]
mod literal_test;
mod online_backup;
#[cfg(test)]
mod online_backup_test;
mod pager;
#[cfg(test)]
mod pager_test;
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
pub enum FileFormat {
    #[clap(name = "csv")]
    CSV,
    #[clap(name = "tsv")]
    TSV,
    #[clap(name = "json")]
    JSON,
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
        format: FileFormat,
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
        format: FileFormat,
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
            con.execute("INSERT INTO t VALUES (?)", &["ok"]).unwrap();
            assert_eq!(
                con.query_row("SELECT v FROM t", [], |row| row.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
            writeln!(&mut stdout, "db-driver-rs {}", env!("CARGO_PKG_VERSION")).expect("writeln! failed.");
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

            // Create a server with the specified driver
            let mut db =
                sqlite3_driver::SQLite3Driver::connect(&database_filepath, READ_ONLY, &sql_cipher_key).unwrap();

            let (command_sender, command_receiver) = std::sync::mpsc::channel::<String>();
            let abort_signal = db.abort_signal();
            let _thread = std::thread::spawn(move || {
                let mut stdin = stdin();
                loop {
                    let mut command = String::new();
                    match stdin.read_line(&mut command) {
                        Err(_) => return,
                        Ok(0) => return,
                        _ => {}
                    }
                    command = command.trim().to_owned();
                    if command == "abort" {
                        abort_signal.store(true, std::sync::atomic::Ordering::SeqCst);
                        continue;
                    }
                    if command_sender.send(command).is_err() {
                        return;
                    };
                }
            });

            // Start the main loop
            loop {
                let Ok(command) = command_receiver.recv() else {
                    return 0;
                };

                // Open request and response files
                let mut r = File::open(&request_body_filepath).unwrap();
                let mut w = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&response_body_filepath)
                    .unwrap();

                fn finish<T: Write, U: Write>(mut stdout: &mut T, w: &mut U, code: usize) {
                    w.flush().expect("Failed to flush the writer.");
                    writeln!(&mut stdout, "{code}").expect("writeln! failed.");
                    stdout.flush().unwrap();
                }

                // Handle the different commands
                match command.as_str() {
                    // Terminate the loop
                    "close" => return 0,

                    "try_reconnect" => {
                        match sqlite3_driver::SQLite3Driver::connect_with_abort_signal(
                            &database_filepath,
                            READ_ONLY,
                            &sql_cipher_key,
                            db.abort_signal(),
                        ) {
                            Ok(new_db) => {
                                db = new_db;
                                write_named(&mut w, &None::<&i64>).expect("Failed to write the result.");
                                finish(&mut stdout, &mut w, 200);
                            }
                            Err(err) => {
                                w.truncate_all();
                                write!(w, "{err}").expect("Failed to write an error message.");
                                finish(&mut stdout, &mut w, 400);
                            }
                        }
                    }

                    // Handle the request
                    "handle" => {
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
                                finish(&mut stdout, &mut w, 400);
                                continue;
                            }
                        };

                        match db.handle(&mut w, &req.query, &req.params, req.mode) {
                            Ok(_) => {
                                finish(&mut stdout, &mut w, 200);
                            }
                            Err(err) => {
                                w.truncate_all();
                                write!(w, "{err}").expect("Failed to write an error message.");
                                finish(&mut stdout, &mut w, 400);
                            }
                        }
                    }

                    // Tokenize, get code lenses, or check syntax
                    "semantic_highlight" | "code_lens" | "check_syntax" => {
                        // Handle the command
                        if let Err(err) = rmp_serde::from_read(r).map(|Query { query }: Query| -> anyhow::Result<()> {
                            match command.trim() {
                                "semantic_highlight" => {
                                    write_named(&mut w, &semantic_highlight::semantic_highlight(&query))?
                                }
                                "code_lens" => write_named(&mut w, &code_lens::code_lens(&query))?,
                                "check_syntax" => write_named(&mut w, &check_syntax::check_syntax(&query)?)?,
                                _ => panic!("Unexpected command {:?}", command),
                            };
                            Ok(())
                        }) {
                            w.flush().unwrap();
                            w.truncate_all();
                            write!(w, "{err:?}").unwrap();
                            finish(&mut stdout, &mut w, 400);
                        } else {
                            w.flush().unwrap();
                            finish(&mut stdout, &mut w, 200);
                        }
                    }

                    // Completion
                    "completion" => {
                        // Handle the command
                        if let Err(err) = rmp_serde::from_read(r).map(
                            |CompletionQuery { sql, line, column }: CompletionQuery| -> anyhow::Result<()> {
                                write_named(
                                    &mut w,
                                    &completion::complete(
                                        &db,
                                        &sql,
                                        &ZeroIndexedLocation {
                                            line: line as usize,
                                            column: column as usize,
                                        },
                                    ),
                                )?;
                                Ok(())
                            },
                        ) {
                            w.flush().unwrap();
                            w.truncate_all();
                            write!(w, "{err:?}").unwrap();
                            finish(&mut stdout, &mut w, 400);
                        } else {
                            w.flush().unwrap();
                            finish(&mut stdout, &mut w, 200);
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
            if let Err(err) = export::export(
                &database_filepath,
                &sql_cipher_key,
                &query,
                format,
                &csv_delimiter,
                output_file,
            ) {
                writeln!(&mut stderr, "{}", err).expect("writeln! failed.");
                return 1;
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
            if let Err(err) = import::import(
                &database_filepath,
                &sql_cipher_key,
                format,
                &table_name,
                &csv_delimiter,
                input_file,
            ) {
                writeln!(&mut stderr, "{}", err).expect("writeln! failed.");
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
    fn truncate_all(&mut self) -> ();
}

impl TruncateAll for std::fs::File {
    fn truncate_all(&mut self) -> () {
        self.set_len(0).expect("Failed to truncate the file.");
        self.seek(SeekFrom::Start(0)).expect("Failed to seek the file.");
    }
}

impl TruncateAll for std::io::Cursor<Vec<u8>> {
    fn truncate_all(&mut self) -> () {
        self.get_mut().truncate(0);
        self.seek(SeekFrom::Start(0)).expect("Failed to seek the cursor.");
    }
}
