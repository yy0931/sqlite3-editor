use rusqlite::InterruptHandle;
use serde::Serialize;

use super::cache::Pager;
use super::find_widget_functions::register_sqlite_functions_for_find_widget;
use crate::cli_error::CLIError;
use crate::cli_subcommands::server::server_commands::query::request_type::QueryMode;
use crate::cli_subcommands::server::server_commands::query::request_type::QueryOptions;
use crate::cli_subcommands::server::server_commands::query::sqlite3::add_sleep_fn::add_sleep_fn;
use crate::cli_subcommands::server::server_commands::query::sqlite3::cache::Records;
use crate::cli_subcommands::server::server_commands::query::sqlite3::columnar_buffer::ColumnarBuffer;
use crate::cli_subcommands::server::sqlite3_fns::assert_readonly_query::assert_readonly_query;
use crate::cli_subcommands::server::sqlite3_fns::list_references::list_references;
use crate::cli_subcommands::server::sqlite3_fns::list_tables::list_tables;
use crate::cli_subcommands::server::sqlite3_fns::query_schema::query_schema;
use crate::cli_subcommands::server::sqlite3_fns::table_schema::table_schema;
use crate::cli_value::CLIValue;
use crate::msgpack::encode_value_ref_into_msgpack;
use crate::msgpack::MessagePackRecord;
use crate::utf8_extractor::InvalidUTF8;
use std::io::Write;
use std::mem::ManuallyDrop;
use std::rc::Rc;

/// A rusqlite's Connection with the following features:
///
/// - Executes a dummy query on `Drop` to clean up the journal files.
/// - Caches the records retrieved with `SELECT ... LIMIT ? OFFSET ?`.
/// - Has the custom SQLite functions for use in the find widget.
pub struct SQLite3Connection {
    con: ManuallyDrop<rusqlite::Connection>,
    pager: Pager,
    pub database_label: String,
}

impl SQLite3Connection {
    /// Connects to the database, set busy_timeout to 500, register the find_widget_compare_r function, enable loading extensions, and fetch the version number of SQLite.
    /// * `read_only` - If true, connects to the database with immutable=1 and the readonly flag. Use this argument to read a database that is under an EXCLUSIVE lock.
    pub fn connect(database_filepath: &str, read_only: bool) -> std::result::Result<Self, CLIError> {
        // Connect to the database
        let con = if !read_only {
            rusqlite::Connection::open(database_filepath)
                .or_else(|err| CLIError::new_ffi_error(err, "sqlite3_open_v2", &[database_filepath.into()]))?
        } else {
            connect_immutable(database_filepath)?
        };

        // Set busy_timeout to 500
        con.pragma_update(None, "busy_timeout", 500)
            .expect("Could not update the busy_timeout value");

        // Register custom SQLite functions
        register_sqlite_functions_for_find_widget(&con)?;

        // Allow loading extensions
        unsafe {
            con.load_extension_enable()
                .expect("Failed to enable loading run-time loadable extensions.");
        }

        let database_label = format!("sqlite {}", rusqlite::version());

        // Get the SQLite version string
        Ok(Self {
            con: ManuallyDrop::new(con),
            pager: Pager::new(),
            database_label,
        })
    }

    pub fn handle(
        &mut self,
        mut w: &mut dyn Write,
        query: &str,
        params: &[CLIValue],
        mode: QueryMode,
        options: QueryOptions,
    ) -> std::result::Result<(), CLIError> {
        let start_time = std::time::Instant::now();

        fn write_editor_pragma<T: Serialize>(
            w: &mut (impl Write + ?Sized),
            data: (T, Vec<InvalidUTF8>),
            start_time: std::time::Instant,
        ) {
            w.write_all(
                &rmp_serde::to_vec_named(&EditorPragmaResponse {
                    data: data.0,
                    warnings: data.1,
                    time: start_time.elapsed().as_secs_f64(),
                })
                .expect("Failed to write msgpack"),
            )
            .expect("Failed to write msgpack");
        }

        match query {
            "EDITOR_PRAGMA database_label" => write_editor_pragma(w, (self.database_label.clone(), vec![]), start_time),
            "EDITOR_PRAGMA list_tables" => write_editor_pragma(w, list_tables(&self.con)?, start_time),
            "EDITOR_PRAGMA list_references" => {
                let (Some(CLIValue::String(table_name)), Some(CLIValue::String(column_name)), Some(value)) =
                    (params.first(), params.get(1), params.get(2))
                else {
                    return CLIError::new_other_error(
                        "invalid arguments for list_references",
                        Some(query.to_owned()),
                        Some(params),
                    );
                };

                write_editor_pragma(
                    w,
                    list_references(&self.con, table_name, column_name, value)?,
                    start_time,
                )
            }
            "EDITOR_PRAGMA table_schema" => {
                let (Some(CLIValue::String(database)), Some(CLIValue::String(table_name))) =
                    (params.first(), params.get(1))
                else {
                    return CLIError::new_other_error(
                        "invalid arguments table_schema",
                        Some(query.to_owned()),
                        Some(params),
                    );
                };
                write_editor_pragma(w, table_schema(&self.con, database, table_name)?, start_time)
            }
            "EDITOR_PRAGMA query_schema" => {
                let Some(CLIValue::String(query)) = params.first() else {
                    return CLIError::new_other_error(
                        "invalid argument for query_schema",
                        Some(query.to_owned()),
                        Some(params),
                    );
                };

                write_editor_pragma(w, query_schema(&self.con, query)?, start_time)
            }
            "EDITOR_PRAGMA total_cache_size_bytes" => {
                write_editor_pragma(w, (self.pager.total_cache_size_bytes(), vec![]), start_time)
            }
            "EDITOR_PRAGMA load_extensions" => {
                let mut extensions = vec![];
                for param in params {
                    if let CLIValue::String(param) = param {
                        extensions.push(param.as_str());
                    }
                }
                write_editor_pragma(w, (self.load_extensions(&extensions)?, vec![]), start_time)
            }
            "EDITOR_PRAGMA add_sleep_fn" => {
                add_sleep_fn(&self.con);
                write_editor_pragma(w, (0, vec![]), start_time)
            }

            _ => {
                if mode == QueryMode::ReadOnly {
                    assert_readonly_query(query, &options.pre_stmt)?;
                }

                let mut warnings = vec![];
                let mut map = MessagePackRecord::new();

                // Write records
                match mode {
                    QueryMode::Script => {
                        assert!(params.is_empty());
                        self.pager.clear_cache();
                        let result = self.con.execute_batch(query);

                        // Rollback uncommitted transactions
                        let _ = self.con.execute("ROLLBACK;", ());

                        result.or_else(|err| CLIError::new_query_error(err, query, params))?;
                        map.insert_value("records", &None::<String>);
                    }
                    QueryMode::ReadOnly => {
                        let buf = self.execute(query, params, ExecMode::ReadOnly, options, &mut warnings)?;
                        map.insert_message_pack("records", buf);
                    }
                    QueryMode::ReadWrite => {
                        let buf = self.execute(query, params, ExecMode::ReadWrite, options, &mut warnings)?;
                        map.insert_message_pack("records", buf);
                    }
                }

                map.insert_value("warnings", &warnings);
                map.insert_value("time", &start_time.elapsed().as_secs_f64());
                map.write_to(&mut w);
            }
        }

        Ok(())
    }

    pub fn con(&self) -> &rusqlite::Connection {
        &self.con
    }

    pub fn get_interrupt_handle(&self) -> InterruptHandle {
        self.con.get_interrupt_handle()
    }

    /// Executes a SQL statement and returns the result as a msgpack.
    fn execute(
        &mut self,
        query: &str,
        params: &[CLIValue],
        read_only: ExecMode,
        options: QueryOptions,
        warnings: &mut Vec<InvalidUTF8>,
    ) -> std::result::Result<Vec<u8>, CLIError> {
        if read_only == ExecMode::ReadOnly {
            assert_readonly_query(query, &options.pre_stmt)?;
        } else {
            self.pager.clear_cache();
        }

        let records = if let Some(records) = self
            .pager
            .query(&mut self.con, query, params, |err| warnings.push(err.with(query)))?
        {
            // Return the cache entry if it exists.
            records
        } else {
            // Prepare the statement
            let tx = self
                .con
                .transaction()
                .or_else(|err| CLIError::new_query_error(err, query, params))?;

            // Pre-query
            if let Some(pre_stmt_str) = options.pre_stmt {
                let mut pre_stmt = tx
                    .prepare(&pre_stmt_str)
                    .or_else(|err| CLIError::new_query_error(err, &pre_stmt_str, params))?;

                for (i, param) in params.iter().enumerate() {
                    pre_stmt
                        .raw_bind_parameter(i + 1, param)
                        .or_else(|err| CLIError::new_query_error(err, &pre_stmt_str, params))?;
                }
                pre_stmt
                    .raw_execute()
                    .or_else(|err| CLIError::new_query_error(err, &pre_stmt_str, params))?;
            }

            let mut stmt = tx
                .prepare(query)
                .or_else(|err| CLIError::new_query_error(err, query, params))?;

            // Bind parameters
            if params.len() != stmt.parameter_count() {
                return Err(CLIError::InvalidNumberOfParameters {
                    query: query.to_string(),
                    placeholders: (1..=stmt.parameter_count())
                        .map(|i| stmt.parameter_name(i).map(|v| v.to_owned()))
                        .collect(),
                    params: params.to_vec(),
                });
            }
            if let Some(placeholders) = options.check_placeholders {
                if placeholders.len() != stmt.parameter_count()
                    || (0..placeholders.len()).any(|i| placeholders[i].as_deref() != stmt.parameter_name(i + 1))
                {
                    return CLIError::new_other_error(
                        format!(
                            "Failed to parse the SQL statement: {placeholders:?} != {:?}",
                            (0..placeholders.len())
                                .map(|i| stmt.parameter_name(i + 1))
                                .collect::<Vec<_>>()
                        ),
                        Some(query.to_owned()),
                        Some(params),
                    );
                }
            }
            for (i, param) in params.iter().enumerate() {
                stmt.raw_bind_parameter(i + 1, param)
                    .or_else(|err| CLIError::new_query_error(err, query, params))?;
            }

            // Fetch records
            let mut col_buf = ColumnarBuffer::default();

            let mut rows = stmt.raw_query();
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        // NOTE: We need to call `stmt.column_count()` after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53),
                        //       but since the borrow checker prevents us from calling `stmt.column_count()` while `row` is alive,
                        //       we rely on `rusqlite::Error::InvalidColumnIndex` returned from `row.get_ref(i)` to check the number of columns.
                        for i in 0usize..=usize::MAX {
                            match row.get_ref(i) {
                                Ok(value) => {
                                    col_buf
                                        .get_column(i)
                                        .push_message_pack(encode_value_ref_into_msgpack(value, |err| {
                                            warnings.push(err.with(query))
                                        }));
                                }
                                Err(rusqlite::Error::InvalidColumnIndex(_)) => break,
                                Err(err) => return CLIError::new_query_error(err, query, params),
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(err) => CLIError::new_query_error(err, query, params)?,
                }
            }

            drop(rows);

            // NOTE: We need to call `stmt.column_names()` after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53)
            let columns = stmt
                .column_names()
                .into_iter()
                .map(|v| v.to_owned())
                .collect::<Vec<_>>();

            drop(stmt);

            if let Some(changes) = options.changes {
                let actual_changes = tx.changes();
                if !if options.allow_fewer_changes {
                    actual_changes <= changes
                } else {
                    actual_changes == changes
                } {
                    tx.rollback()
                        .or_else(|err| CLIError::new_query_error(err, query, params))?;
                    return Err(CLIError::UnexpectedChanges {
                        expected: changes,
                        actual: actual_changes,
                        query: query.to_owned(),
                        params: params.to_owned(),
                    });
                }
            }

            // don't commit on readonly connections
            if read_only == ExecMode::ReadOnly {
                tx.rollback()
            } else {
                tx.commit()
            }
            .or_else(|err| CLIError::new_query_error(err, query, params))?;

            Records::new(col_buf.finish(columns.len()), Rc::new(columns))
        };

        // Pack the result into a msgpack
        let mut map = MessagePackRecord::new();
        for (i, column_name) in records.columns().iter().enumerate() {
            map.insert_message_pack(column_name, records.col_buf()[i].to_vec());
        }
        Ok(map.to_vec())
    }

    pub fn load_extensions(&self, extensions: &[&str]) -> std::result::Result<(), CLIError> {
        for ext in extensions {
            unsafe {
                self.con.load_extension(ext, None).or_else(|err| {
                    CLIError::new_ffi_error(
                        err,
                        "sqlite3_load_extension",
                        &extensions.iter().map(|v| (*v).into()).collect::<Vec<CLIValue>>(),
                    )
                })?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn pager(&mut self) -> &mut Pager {
        &mut self.pager
    }

    #[cfg(test)]
    pub fn _execute(
        &mut self,
        query: &str,
        params: &[CLIValue],
        read_only: ExecMode,
        options: QueryOptions,
        warnings: &mut Vec<InvalidUTF8>,
    ) -> std::result::Result<Vec<u8>, CLIError> {
        self.execute(query, params, read_only, options, warnings)
    }
}

impl Drop for SQLite3Connection {
    fn drop(&mut self) {
        // Executes a dummy query to clean up the journal files.
        if let Ok(mut stmt) = self.con.prepare("SELECT * FROM sqlite_schema LIMIT 1") {
            let _ = stmt.query(());
        }
        unsafe {
            let _ = ManuallyDrop::take(&mut self.con);
        };
    }
}

fn connect_immutable(database_filepath: &str) -> std::result::Result<rusqlite::Connection, CLIError> {
    // Connect to the database with `?immutable=1` and the readonly flag
    const ASCII_SET: percent_encoding::AsciiSet = percent_encoding::NON_ALPHANUMERIC.remove(b'/');
    rusqlite::Connection::open_with_flags(
        format!(
            "file:{}?immutable=1",
            percent_encoding::utf8_percent_encode(database_filepath, &ASCII_SET)
        ),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .or_else(|err| CLIError::new_ffi_error(err, "sqlite3_open_v2 (immutable=1)", &[database_filepath.into()]))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Debug, Serialize, ts_rs::TS)]
#[ts(export)]
struct EditorPragmaResponse<T: Serialize> {
    data: T,
    warnings: Vec<InvalidUTF8>,
    time: f64,
}
