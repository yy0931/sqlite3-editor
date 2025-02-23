use std::{
    io::{Cursor, Write},
    sync::mpsc::{Receiver, Sender},
    thread::JoinHandle,
};

use tempfile::NamedTempFile;

use crate::{
    cli,
    request_type::{QueryMode, Request},
    sqlite3::QueryOptions,
    Args, ExportFormat, Query, ReadCommand, ServerCommand,
};

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

#[test]
fn test_version() {
    let mut stdout = vec![];
    assert_eq!(
        cli(
            Args {
                command: crate::Commands::Version {},
            },
            || Cursor::new("".to_owned()),
            &mut stdout,
            &mut vec![],
        ),
        0
    );
    assert!(String::from_utf8(stdout).unwrap().starts_with("sqlite3-editor "));
}

#[test]
fn test_function_list() {
    let mut stdout = vec![];
    assert_eq!(
        cli(
            Args {
                command: crate::Commands::FunctionList {},
            },
            || Cursor::new("".to_owned()),
            &mut stdout,
            &mut vec![],
        ),
        0
    );
    let json = String::from_utf8(stdout).unwrap();
    let json = json.trim();
    assert!(json.starts_with("[\""));
    assert!(json.ends_with("\"]"));
}

fn test_export_to_stdout(format: ExportFormat) -> String {
    let f = NamedTempFile::new().unwrap();

    let conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x, y)", ()).unwrap();
    conn.execute("INSERT INTO t VALUES (1, 2), (3, 4)", ()).unwrap();

    let mut stdout = vec![];
    assert_eq!(
        cli(
            Args {
                command: crate::Commands::Export {
                    database_filepath: f.path().to_str().unwrap().to_owned(),
                    sql_cipher_key: None,
                    format,
                    query: "SELECT * FROM t".to_owned(),
                    output_file: None,
                    xlsx_options: None,
                    csv_options: None,
                },
            },
            || Cursor::new("".to_owned()),
            &mut stdout,
            &mut vec![],
        ),
        0
    );
    String::from_utf8(stdout).unwrap()
}

#[test]
fn test_export_csv() {
    assert_eq!(test_export_to_stdout(ExportFormat::CSV), "x,y\n1,2\n3,4\n");
}

#[test]
fn test_export_json() {
    assert_eq!(
        test_export_to_stdout(ExportFormat::JSON),
        "[{\"x\":1,\"y\":2},{\"x\":3,\"y\":4}]"
    );
}

fn wait_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
struct SenderWriter {
    sender: std::sync::mpsc::Sender<Vec<u8>>,
    buf: Vec<u8>,
}

impl SenderWriter {
    fn new(sender: Sender<Vec<u8>>) -> Self {
        Self {
            sender,
            buf: Vec::new(),
        }
    }
}

impl Write for SenderWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sender.send(std::mem::take(&mut self.buf)).unwrap();
        Ok(())
    }
}

struct ReceiverReader {
    receiver: std::sync::mpsc::Receiver<ServerCommand>,
}

impl ReceiverReader {
    fn new(receiver: std::sync::mpsc::Receiver<ServerCommand>) -> Self {
        ReceiverReader { receiver }
    }
}

impl ReadCommand for ReceiverReader {
    fn read_command(&mut self) -> Option<ServerCommand> {
        self.receiver.recv().ok()
    }
}

struct ServerTestBench {
    server_thread: JoinHandle<()>,
    #[allow(dead_code)]
    database_filepath: NamedTempFile,
    request_body_filepath: NamedTempFile,
    response_body_filepath: NamedTempFile,
    stdin_sender: Sender<ServerCommand>,
    stdout_receiver: Receiver<Vec<u8>>,
    #[allow(dead_code)]
    stderr_receiver: Receiver<Vec<u8>>,
}

impl ServerTestBench {
    fn new() -> Self {
        let database_filepath = NamedTempFile::new().unwrap();
        let request_body_filepath = NamedTempFile::new().unwrap();
        let response_body_filepath = NamedTempFile::new().unwrap();
        let (stdin_sender, stdin_receiver) = std::sync::mpsc::channel::<ServerCommand>();
        let (stdout_sender, stdout_receiver) = std::sync::mpsc::channel::<Vec<u8>>();
        let mut stdout_writer = SenderWriter::new(stdout_sender);
        let (stderr_sender, stderr_receiver) = std::sync::mpsc::channel::<Vec<u8>>();
        let mut stderr_writer = SenderWriter::new(stderr_sender);

        Self {
            server_thread: {
                let database_filepath = database_filepath.path().to_str().unwrap().to_owned();
                let request_body_filepath = request_body_filepath.path().to_owned();
                let response_body_filepath = response_body_filepath.path().to_owned();
                std::thread::spawn(move || {
                    assert_eq!(
                        cli(
                            Args {
                                command: crate::Commands::Server {
                                    database_filepath,
                                    request_body_filepath,
                                    response_body_filepath,
                                    sql_cipher_key: None,
                                },
                            },
                            move || { ReceiverReader::new(stdin_receiver) },
                            &mut stdout_writer,
                            &mut stderr_writer,
                        ),
                        0
                    );
                })
            },
            database_filepath,
            request_body_filepath,
            response_body_filepath,
            stdin_sender,
            stdout_receiver,
            stderr_receiver,
        }
    }

    fn send_stdin(&self, command: ServerCommand) {
        self.stdin_sender.send(command).unwrap();
    }

    fn recv_stdout(&self) -> String {
        String::from_utf8(self.stdout_receiver.recv().unwrap()).unwrap()
    }

    #[allow(dead_code)]
    fn recv_stderr(&self) -> String {
        String::from_utf8(self.stderr_receiver.recv().unwrap()).unwrap()
    }

    fn write_request_body<T>(&self, data: &T)
    where
        T: serde::Serialize + ?Sized,
    {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.request_body_filepath)
            .unwrap()
            .write_all(&rmp_serde::to_vec(&data).unwrap())
            .unwrap();
    }

    fn read_response_body(&self) -> String {
        std::fs::read_to_string(&self.response_body_filepath).unwrap()
    }
}

impl Drop for ServerTestBench {
    fn drop(&mut self) {
        // .send() returns an Error if the server is already shut down.
        let _ = self.stdin_sender.send(ServerCommand::Close);

        // sever_thread.join() without taking its ownership
        while !self.server_thread.is_finished() {
            wait_ms(1);
        }
    }
}

#[test]
fn test_server_close() {
    let test_bench = ServerTestBench::new();
    wait_ms(100);
    assert!(!test_bench.server_thread.is_finished());
    test_bench.send_stdin(ServerCommand::Close);
    wait_ms(100);
    assert!(test_bench.server_thread.is_finished());
}

#[test]
fn test_server_handle() {
    let test_bench = ServerTestBench::new();
    test_bench.write_request_body(&Request {
        query: "SELECT 1".to_owned(),
        params: vec![],
        mode: QueryMode::ReadOnly,
        options: QueryOptions::default(),
    });
    test_bench.send_stdin(ServerCommand::Handle);
    assert_eq!(test_bench.recv_stdout(), "Success\n");
}

#[test]
#[cfg(not(feature = "sqlcipher"))] // sleep() does not support interruption under feature="sqlcipher"
fn test_server_interrupt() {
    let test_bench = ServerTestBench::new();

    test_bench.write_request_body(&Request {
        query: "EDITOR_PRAGMA add_sleep_fn".to_owned(),
        params: vec![],
        mode: QueryMode::ReadOnly,
        options: QueryOptions::default(),
    });
    test_bench.send_stdin(ServerCommand::Handle);
    assert_eq!(test_bench.recv_stdout(), "Success\n");

    test_bench.write_request_body(&Request {
        query: "SELECT sleep(5000)".to_owned(),
        params: vec![],
        mode: QueryMode::ReadOnly,
        options: QueryOptions::default(),
    });
    test_bench.send_stdin(ServerCommand::Handle);
    wait_ms(500);
    test_bench.send_stdin(ServerCommand::Interrupt);
    assert_eq!(test_bench.recv_stdout(), "OtherError\n");
    assert_eq!(
        test_bench.read_response_body(),
        "interrupted\nQuery: SELECT sleep(5000)\nParameters: []END"
    );
}

#[test]
fn test_server_disconnect_temporarily() {
    let test_bench = ServerTestBench::new();

    test_bench.send_stdin(ServerCommand::DisconnectTemporarily);
    assert_eq!(test_bench.recv_stdout(), "Success\n");

    test_bench.send_stdin(ServerCommand::Resume);
    assert_eq!(test_bench.recv_stdout(), "Success\n");

    test_bench.write_request_body(&Request {
        query: "SELECT 1".to_owned(),
        params: vec![],
        mode: QueryMode::ReadOnly,
        options: QueryOptions::default(),
    });
    test_bench.send_stdin(ServerCommand::Handle);
    assert_eq!(test_bench.recv_stdout(), "Success\n");
}
