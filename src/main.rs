use std::{
    borrow::Cow,
    fs::File,
    io::{Read, Seek, Write},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
mod export;
mod import;

use crate::types::{Request, TruncateAll};

mod check_syntax;
mod code_lens;
#[cfg(test)]
mod code_lens_test;
#[cfg(test)]
mod export_test;
#[cfg(test)]
mod import_test;
mod parse_cte;
#[cfg(test)]
mod parse_cte_test;
mod semantic_highlight;
#[cfg(test)]
mod semantic_highlight_test;
mod split_statements;
#[cfg(test)]
mod split_statements_test;
mod sqlite3_driver;
mod tokenize;
#[cfg(test)]
mod tokenize_test;
mod types;

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
    HealthCheck {},
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

#[test]
fn test_parse_query() {
    let q: Query = serde_json::from_str("[\"foo\"]").unwrap();
    assert_eq!(
        q,
        Query {
            query: "foo".to_owned()
        }
    );
}

fn main() {
    // Parse the command line arguments
    let args = Args::parse();
    match args.command {
        Commands::Server {
            database_filepath,
            request_body_filepath,
            sql_cipher_key,
            response_body_filepath,
        } => {
            // Create a server with the specified driver
            let server = sqlite3_driver::SQLite3Driver::connect(&database_filepath, false, &sql_cipher_key).unwrap();

            // Start the main loop
            let stdin = std::io::stdin();
            loop {
                let mut command = String::new();

                // Exit the loop if reading from stdin fails
                if let Err(_) = stdin.read_line(&mut command) {
                    return;
                };

                // Open request and response files
                let mut r = File::open(&request_body_filepath).unwrap();
                let mut w = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&response_body_filepath)
                    .unwrap();

                fn finish<W: Write>(w: &mut W, code: usize) {
                    w.flush().expect("Failed to flush the writer.");
                    println!("{code}");
                    std::io::stdout().flush().unwrap();
                }

                fn inspect_request(mut r: impl Read + Seek) -> Cow<'static, str> {
                    r.rewind().expect("Failed to rewind the reader.");
                    let json = vec![];
                    match String::from_utf8(json.clone()) {
                        Ok(v)
                            if serde_transcode::transcode(
                                &mut rmp_serde::Deserializer::new(&mut r),
                                &mut serde_json::Serializer::new(json),
                            )
                            .is_ok() =>
                        {
                            Cow::Owned(v)
                        }
                        _ => Cow::Borrowed("<Failed to serialize as a JSON>"),
                    }
                }

                // Handle the different commands
                match command.trim() {
                    // Terminate the loop
                    "close" => return,

                    // Handle the request using the MsgpackServer
                    "handle" => {
                        // Deserialize the request
                        let Ok(req) = rmp_serde::from_read::<_, Request>(&mut r) else {
                            w.write(format!("Failed to parse the request body: {}", inspect_request(r)).as_bytes())
                                .expect("Failed to write an error message.");
                            finish(&mut w, 400);
                            continue;
                        };

                        match server.handle(&mut w, &req.query, &req.params, req.mode) {
                            Err(err) => {
                                w.truncate_all();
                                w.write(
                                    format!(
                                        "{err}\n{}\nParams: {:?}",
                                        if req.query.starts_with("EDITOR_PRAGMA ") {
                                            format!("Method: {}", &req.query["EDITOR_PRAGMA ".len()..])
                                        } else {
                                            format!("Query: {}", req.query)
                                        },
                                        req.params
                                    )
                                    .as_bytes(),
                                )
                                .expect("Failed to write an error message.");
                                finish(&mut w, 400);
                            }
                            Ok(_) => {
                                finish(&mut w, 200);
                            }
                        }
                    }

                    // Tokenize, get code lenses, or check syntax
                    "semantic_highlight" | "code_lens" | "check_syntax" => {
                        // Handle the command
                        if let Err(err) = rmp_serde::from_read(r).map(|Query { query }: Query| -> anyhow::Result<()> {
                            use rmp_serde::encode::write_named;
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
                            w.set_len(0).unwrap();
                            w.write_fmt(format_args!("{err:?}")).unwrap();
                            finish(&mut w, 400);
                        } else {
                            w.flush().unwrap();
                            finish(&mut w, 200);
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
                eprintln!("{}", err);
                std::process::exit(1);
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
                eprintln!("{}", err);
                std::process::exit(1);
            }
        }
        Commands::HealthCheck {} => {
            let mem = rusqlite::Connection::open_in_memory().unwrap();
            assert_eq!(
                mem.query_row("SELECT 'ok'", [], |row| row.get::<_, String>(0)).unwrap(),
                "ok"
            );
        }
    }
}
