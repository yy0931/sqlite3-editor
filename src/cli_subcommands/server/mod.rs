mod server_commands;
mod sqlite3_fns;
mod sqlite3_query_parser;

#[cfg(test)]
pub mod _server_commands {
    pub use super::server_commands::*;
}

use rmp_serde::encode::write_named;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::BufRead;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crate::cli_error::CLIErrorCode;
use crate::cli_subcommands::server::server_commands::query::sqlite3::connection::SQLite3Connection;

pub fn run<F, I>(
    stdin: F,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    database_filepath: String,
    request_body_filepath: PathBuf,
    response_body_filepath: PathBuf,
) -> i32
where
    F: FnOnce() -> I + std::marker::Send + 'static,
    I: ReadCommand,
{
    const READ_ONLY: bool = false;

    // Create a server
    let mut db = match SQLite3Connection::connect(&database_filepath, READ_ONLY) {
        Ok(db) => db,
        Err(err) => {
            writeln!(stderr, "{err}").expect("writeln! failed.");
            return 1;
        }
    };

    let (command_sender, command_receiver) = std::sync::mpsc::channel::<ServerCommand>();
    let (resume_command_sender, resume_command_receiver) = std::sync::mpsc::channel::<()>();
    let interrupt_handle = Arc::new(Mutex::new(db.get_interrupt_handle()));
    let _thread = {
        let interrupt_handle = Arc::clone(&interrupt_handle);
        std::thread::spawn(move || {
            let mut stdin = stdin();
            loop {
                match stdin.read_command() {
                    Some(ServerCommand::Interrupt) => {
                        interrupt_handle.lock().unwrap().interrupt();
                    }
                    Some(ServerCommand::Resume) => {
                        resume_command_sender.send(()).unwrap();
                    }
                    Some(command) => {
                        if command_sender.send(command).is_err() {
                            return;
                        }
                    }
                    None => return,
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
        let mut r = File::open(&request_body_filepath)
            .unwrap_or_else(|err| panic!("unable to open database file {}: {err:?}", request_body_filepath.to_string_lossy()));
        let mut w = match std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&response_body_filepath)
        {
            Ok(w) => w,
            Err(err) => {
                // Handle `Os { code: 1224, kind: Uncategorized, message: "The requested operation cannot be performed on a file with a user-mapped section open." }`
                if err.kind() == std::io::ErrorKind::Other && err.raw_os_error() == Some(1224) {
                    writeln!(stderr, "Failed to create a temporary file: {err}").expect("writeln! failed.");
                    return 1;
                }
                panic!("{err:?}");
            }
        };

        fn finish<T: Write, U: Write>(mut stdout: &mut T, w: &mut U, code: CLIErrorCode) {
            w.write_all(b"END").expect("Failed to write the result.");
            w.flush().expect("Failed to flush the writer.");
            writeln!(&mut stdout, "{code:?}").expect("writeln! failed.");
            stdout.flush().unwrap();
        }

        // Handle the different commands
        let code = match command {
            ServerCommand::TryReconnect => match SQLite3Connection::connect(&database_filepath, READ_ONLY) {
                Ok(new_db) => {
                    db = new_db;

                    *interrupt_handle.lock().unwrap() = db.get_interrupt_handle();
                    write_named(&mut w, &None::<&i64>).expect("Failed to write the result.");
                    CLIErrorCode::Success
                }
                Err(err) => {
                    w.truncate_all();
                    write!(w, "{err}").expect("Failed to write an error message.");
                    err.code()
                }
            },

            ServerCommand::DisconnectTemporarily => {
                drop(db);

                // Send the response to DisconnectTemporarily
                write_named(&mut w, &None::<&i64>).expect("Failed to write the result.");
                finish(stdout, &mut w, CLIErrorCode::Success);

                if resume_command_receiver.recv().is_err() {
                    return 0;
                }

                match SQLite3Connection::connect(&database_filepath, READ_ONLY) {
                    Ok(new_db) => {
                        db = new_db;
                        *interrupt_handle.lock().unwrap() = db.get_interrupt_handle();

                        // Send the response to Resume
                        write_named(&mut w, &None::<&i64>).expect("Failed to write the result.");
                        CLIErrorCode::Success
                    }
                    Err(err) => {
                        w.truncate_all();
                        write!(w, "{err}").expect("Failed to write an error message.");
                        finish(stdout, &mut w, err.code());
                        return 1;
                    }
                }
            }

            ServerCommand::Query => server_commands::query::run(&mut db, &mut r, &mut w),
            ServerCommand::SemanticHighlight => server_commands::semantic_highlight::run(&mut r, &mut w),
            ServerCommand::CodeLens => server_commands::code_lens::run(&mut r, &mut w),
            ServerCommand::CheckSyntax => server_commands::diagnosis::run(&mut r, &mut w),
            ServerCommand::Completion => server_commands::completion::run(db.con(), &mut r, &mut w),

            // Ignore unrecognized commands
            _ => continue,
        };
        finish(stdout, &mut w, code);
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename_all = "snake_case")]
pub enum ServerCommand {
    Interrupt,
    Close,
    TryReconnect,
    DisconnectTemporarily,
    Resume,
    Query,
    SemanticHighlight,
    CodeLens,
    CheckSyntax,
    Completion,
}

pub trait ReadCommand {
    fn read_command(&mut self) -> Option<ServerCommand>;
}

impl<T: Read + BufRead> ReadCommand for T {
    fn read_command(&mut self) -> Option<ServerCommand> {
        loop {
            let mut command_str = String::new();
            match self.read_line(&mut command_str) {
                Err(_) => return None,
                Ok(0) => return None,
                _ => {}
            }

            if let Ok(command) = ServerCommand::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(
                command_str.trim(),
            )) {
                return Some(command);
            }
        }
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
