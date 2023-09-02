use crate::{literal::Literal, request_type::QueryMode};
use lazy_static::lazy_static;
use rusqlite::{functions::FunctionFlags, types::ValueRef};
use serde::{Deserialize, Serialize};

use std::{
    io::Write,
    mem::ManuallyDrop,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq)]
pub struct QueryError {
    pub message: String,
    pub query: String,
    pub params: Vec<Literal>,
}

impl QueryError {
    pub fn new<T, E: std::fmt::Display>(err: E, query: &str, params: &[Literal]) -> std::result::Result<T, Self> {
        Err(Self {
            message: format!("{}", err),
            query: query.to_owned(),
            params: params.iter().cloned().collect(),
        })
    }
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\n{}\nParams: {}",
            self.message,
            if self.query.starts_with("EDITOR_PRAGMA ") {
                format!("Method: {}", &self.query["EDITOR_PRAGMA ".len()..])
            } else {
                format!("Query: {}", self.query)
            },
            serde_json::to_string(&self.params).unwrap_or("<failed to serialize>".to_owned())
        )
    }
}

pub(crate) struct SQLite3Driver {
    con: ManuallyDrop<rusqlite::Connection>,
}

lazy_static! {
    static ref REGEX_CACHE: Arc<Mutex<(String, regex::Regex)>> = Arc::new(Mutex::<(String, regex::Regex)>::new((
        "".to_owned(),
        regex::Regex::new("").unwrap(),
    )));
    static ref NON_READONLY_SQL_PATTERN: regex::Regex =
        regex::Regex::new("(?i)^(INSERT |DELETE |UPDATE |CREATE |DROP |ALTER TABLE)").unwrap();
}

/// Text matching implementation for the find widget.
/// Returns Ok(0) on error.
fn find_widget_regexp(ctx: &rusqlite::functions::Context) -> std::result::Result<i64, String> {
    // Receive arguments
    let text = match ctx.get_raw(0) {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(v) => format!("{v}"),
        ValueRef::Real(v) => format!("{v}"),
        ValueRef::Text(v) => String::from_utf8_lossy(v).to_string(),
        ValueRef::Blob(_v) => "".to_owned(), // TODO: hex?
    };
    let Ok(pattern) = ctx.get::<String>(1) else {
        return Ok(0);
    };
    let Ok(whole_word) = ctx.get::<i64>(2) else {
        return Ok(0);
    };
    let Ok(case_sensitive) = ctx.get::<i64>(3) else {
        return Ok(0);
    };

    // Match
    let flags = if case_sensitive != 0 { "" } else { "(?i)" };
    let pattern = if whole_word != 0 {
        format!("{flags}(?s)\\b(?:{pattern})\\b")
    } else {
        format!("{flags}{pattern}")
    };
    let regex = {
        let cache_value = REGEX_CACHE.lock().unwrap();
        if cache_value.0 == pattern {
            cache_value.1.clone()
        } else {
            match regex::Regex::new(&pattern) {
                Ok(v) => v,
                Err(_) => return Ok(0),
            }
        }
    }; // drop lock
    let matched = if regex.is_match(&text) { 1 } else { 0 };
    *REGEX_CACHE.lock().unwrap() = (pattern, regex);
    Ok(matched)
}

// TODO: test LoadableSQLiteExtensionNotAvailable
#[derive(Debug)]
struct LoadableSQLiteExtensionNotAvailable {}

impl std::fmt::Display for LoadableSQLiteExtensionNotAvailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Loadable SQLite extension is not available on this binary.")
    }
}

impl std::error::Error for LoadableSQLiteExtensionNotAvailable {}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TableSchemaColumnForeignKey {
    pub id: i64,
    pub seq: i64,
    pub table: String,
    pub from: String,
    pub to: Option<String>,
    pub on_update: String,
    pub on_delete: String,
    #[serde(rename = "match")]
    pub match_: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DfltValue {
    Int(i64),
    Real(f64),
    String(String),
    Blob(Vec<u8>),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TableSchemaColumn {
    pub cid: i64,
    pub dflt_value: Option<DfltValue>,
    pub name: String,
    pub notnull: bool,
    #[serde(rename = "type")]
    pub type_: String,
    pub pk: bool,
    #[serde(rename = "autoIncrement")]
    pub auto_increment: bool,
    #[serde(rename = "foreignKeys")]
    pub foreign_keys: Vec<TableSchemaColumnForeignKey>,
    /** 1: columns in virtual tables, 2: dynamic generated columns, 3: stored generated columns */
    pub hidden: i64,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexColumn {
    pub seqno: i64,
    pub cid: i64,
    pub name: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSchemaIndex {
    pub seq: i64,
    pub name: String,
    pub unique: i64,
    pub origin: String,
    pub partial: i64,
    pub columns: Option<Vec<IndexColumn>>, // None while fetching
    pub schema: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSchemaTriggers {
    pub name: String,
    pub sql: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TableSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub schema: String,
    #[serde(rename = "hasRowIdColumn")]
    pub has_rowid_column: bool,
    pub strict: bool,
    pub columns: Vec<TableSchemaColumn>,
    pub indexes: Vec<TableSchemaIndex>,
    pub triggers: Vec<TableSchemaTriggers>,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct Table {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ForeignKey {
    pub name: String,
    pub table: String,
    pub from: String,
    pub to: String,
}

fn write_value_ref_into_msgpack<W: rmp::encode::RmpWrite>(
    wr: &mut W,
    value: ValueRef,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    match value {
        ValueRef::Null => {
            rmp::encode::write_nil(wr)?;
        }
        ValueRef::Integer(v) => {
            rmp::encode::write_sint(wr, v)?;
        }
        ValueRef::Text(v) => {
            rmp::encode::write_str(wr, &String::from_utf8_lossy(v))?;
        }
        ValueRef::Real(v) => {
            rmp::encode::write_f64(wr, v)?;
        }
        ValueRef::Blob(v) => {
            rmp::encode::write_bin(wr, v)?;
        }
    }
    Ok(())
}

impl SQLite3Driver {
    /// Connects to the database, set busy_timeout to 500, register the find_widget_regexp function, enable loading extensions, and fetch the version number of SQLite.
    /// * `read_only` - If true, connects to the database with immutable=1 and the readonly flag. Use this argument to read a database that is under an EXCLUSIVE lock.
    /// * `sql_cipher_key` - The encryption key for SQLCipher.
    pub fn connect<'a>(
        database_filepath: &str,
        read_only: bool,
        sql_cipher_key: &Option<impl AsRef<str>>,
    ) -> std::result::Result<Self, String> {
        let con = if !read_only {
            // Connect to the database
            rusqlite::Connection::open(database_filepath)
                .or_else(|err| Err(format!("Failed to open the database {database_filepath:?}: {err}")))?
        } else {
            // Connect to the database with `?immutable=1` and the readonly flag
            const ASCII_SET: percent_encoding::AsciiSet = percent_encoding::NON_ALPHANUMERIC.remove(b'/');
            rusqlite::Connection::open_with_flags(
                format!(
                    "file:{}?immutable=1",
                    percent_encoding::utf8_percent_encode(database_filepath, &ASCII_SET)
                ),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
            )
            .or_else(|err| {
                Err(format!(
                    "Failed to open the database {database_filepath:?} with immutable=1: {err}"
                ))
            })?
        };

        // Set the SQLite Cipher key if given
        if let Some(key) = sql_cipher_key {
            set_sqlcipher_key(&con, key.as_ref())?;
        }

        // Set busy_timeout to 500
        con.pragma_update(None, "busy_timeout", 500)
            .expect("Could not update the busy_timeout value");

        // Register the function "find_widget_regexp"
        con.create_scalar_function(
            "find_widget_regexp",
            4,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| find_widget_regexp(ctx).or(Ok(0)),
        )
        .unwrap();

        // Allow loading extensions
        unsafe {
            con.load_extension_enable()
                .expect("Failed to enable loading run-time loadable extensions.");
        }

        // Get the SQLite version as a string
        Ok(Self {
            con: ManuallyDrop::new(con),
        })
    }

    /// Executes a SQL statement and returns the result as a msgpack.
    pub(crate) fn execute(&self, query: &str, params: &[Literal]) -> std::result::Result<Vec<u8>, QueryError> {
        // Prepare the statement
        let mut stmt = self
            .con
            .prepare(query)
            .or_else(|err| QueryError::new(err, query, params))?;

        // Fetch data and pack into msgpack
        let mut col_buf: Vec<Vec<u8>> = vec![];
        let column_count = stmt.column_count();
        for _ in 0..column_count {
            col_buf.push(vec![]);
        }
        for (i, param) in params.iter().enumerate() {
            stmt.raw_bind_parameter(i + 1, param).unwrap();
        }
        let mut num_rows = 0u32;
        stmt.raw_query()
            .mapped(|row| {
                num_rows += 1;
                for i in 0..column_count {
                    write_value_ref_into_msgpack(&mut col_buf[i], row.get_ref_unwrap(i))
                        .expect("Failed to write msgpack");
                }
                Ok(())
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .or_else(|err| QueryError::new(err, query, params))?;
        let mut buf = vec![];
        let column_names = stmt.column_names();
        rmp::encode::write_map_len(&mut buf, column_names.len() as u32).expect("Failed to write msgpack");
        for (i, column_name) in column_names.iter().enumerate() {
            rmp::encode::write_str(&mut buf, column_name).expect("Failed to write msgpack");
            rmp::encode::write_array_len(&mut buf, num_rows).expect("Failed to write msgpack");
            buf.extend(&col_buf[i]);
        }
        Ok(buf)
    }

    /// Executes a SQL statement and returns the result.
    pub(crate) fn select_all<F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>, T>(
        &self,
        query: &str,
        params: &[Literal],
        map: F,
    ) -> std::result::Result<Vec<T>, QueryError> {
        // Prepare the statement
        let mut stmt = self
            .con
            .prepare(query)
            .or_else(|err| QueryError::new(err, query, params))?;

        // Fetch data and pack into Vec<T>
        let mut col_buf: Vec<Vec<u8>> = vec![];
        let column_count = stmt.column_count();
        for _ in 0..column_count {
            col_buf.push(vec![]);
        }
        for (i, param) in params.iter().enumerate() {
            stmt.raw_bind_parameter(i + 1, param)
                .or_else(|err| QueryError::new(err, query, params))?;
        }
        let records = stmt
            .raw_query()
            .mapped::<F, T>(map)
            .collect::<rusqlite::Result<Vec<T>>>()
            .or_else(|err| QueryError::new(err, query, params))?;

        Ok(records)
    }

    /// Executes a SQL statement and returns the first row.
    pub(crate) fn select_one<F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>, T: ToOwned<Owned = T>>(
        &self,
        query: &str,
        params: &[Literal],
        map: F,
    ) -> std::result::Result<T, QueryError> {
        let select_all = self.select_all(query, params, map)?;
        let get = select_all.get(0);
        let Some(one) = get else {
            return QueryError::new("No records are returned.", query, params);
        };
        Ok(one.to_owned())
    }

    pub(crate) fn database_label(&self) -> String {
        format!(
            "{} {}",
            if is_sqlcipher(&self.con) { "sqlcipher" } else { "sqlite" },
            rusqlite::version()
        )
    }

    pub(crate) fn list_tables(&self) -> std::result::Result<Vec<Table>, QueryError> {
        self.select_all(
            r#"SELECT name, type FROM pragma_table_list WHERE NOT (name LIKE "sqlite\_%" ESCAPE "\")"#,
            &[],
            |row| {
                Ok(Table {
                    name: row.get("name")?,
                    type_: row.get("type")?,
                })
            },
        )
    }

    pub(crate) fn list_foreign_keys(&self) -> std::result::Result<Vec<ForeignKey>, QueryError> {
        self.select_all(r#"SELECT name, f."table", f."from", f."to" FROM pragma_table_list JOIN pragma_foreign_key_list(name) f WHERE NOT (name LIKE "sqlite\\_%" ESCAPE "\\");"#, &[], |row| Ok(ForeignKey { name: row.get("name")?, from: row.get("from")?, table: row.get("table")?, to: row.get("to")? }))
    }

    /// Collect table definitions from sqlite_schema, pragma_table_list, pragma_foreign_key_list, pragma_table_xinfo, pragma_index_list, and pragm_index_info.
    pub(crate) fn table_schema(&self, table_name: &str) -> std::result::Result<TableSchema, QueryError> {
        // Select pragma_table_list
        let (table_type, wr, strict) = self.select_one(
            "SELECT type, wr, strict FROM pragma_table_list WHERE name = ?",
            &[Literal::String(table_name.to_owned())],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? != 0,
                    row.get::<_, i64>(2)? != 0,
                ))
            },
        )?;
        let has_rowid_column = table_type == "table" && !wr;

        // Select pragma_foreign_key_list
        let foreign_keys: Vec<TableSchemaColumnForeignKey> = self.select_all(
            &format!("PRAGMA foreign_key_list({})", escape_sql_identifier(table_name)),
            &[],
            |row| {
                Ok(TableSchemaColumnForeignKey {
                    from: row.get("from")?,
                    id: row.get("id")?,
                    match_: row.get("match")?,
                    on_delete: row.get("on_delete")?,
                    on_update: row.get("on_update")?,
                    seq: row.get("seq")?,
                    table: row.get("table")?,
                    to: row.get("to")?,
                })
            },
        )?;

        // Select sqlite_sequence
        // NOTE: There is no way to check if an empty table has an autoincrement column.
        let has_table_auto_increment_column: bool = !self
            .select_all(
                "SELECT name FROM sqlite_schema WHERE type = 'table' AND name = 'sqlite_sequence'",
                &[],
                |_row| Ok(()),
            )?
            .is_empty()
            && !self
                .select_all(
                    "SELECT * FROM sqlite_sequence WHERE name = ?",
                    &[Literal::String(table_name.to_owned())],
                    |_row| Ok(()),
                )?
                .is_empty();

        let get_sql_column = |records: Option<Vec<(Option<std::string::String>,)>>| -> Option<String> {
            if let Some(records) = records {
                if !records.is_empty() {
                    return records[0].0.clone();
                }
            }
            None
        };

        // Select pragma_table_xinfo
        let columns: Vec<TableSchemaColumn> = self.select_all(
            &format!("PRAGMA table_xinfo({})", escape_sql_identifier(table_name)),
            &[],
            |row| {
                let name = row.get::<_, String>("name")?;
                let pk = row.get::<_, i64>("pk")? != 0;
                Ok(TableSchemaColumn {
                    cid: row.get("cid")?,
                    notnull: row.get::<_, i64>("notnull")? != 0,
                    type_: row.get("type")?,
                    pk,
                    auto_increment: pk && has_table_auto_increment_column,
                    foreign_keys: foreign_keys.clone().into_iter().filter(|k| k.from == name).collect(),
                    hidden: row.get::<_, i64>("hidden")?,
                    dflt_value: match row.get_ref("dflt_value")? {
                        ValueRef::Null => None,
                        ValueRef::Integer(v) => Some(DfltValue::Int(v)),
                        ValueRef::Real(v) => Some(DfltValue::Real(v)),
                        ValueRef::Text(v) => Some(DfltValue::String(String::from_utf8_lossy(v).to_string())),
                        ValueRef::Blob(v) => Some(DfltValue::Blob(v.to_owned())),
                    },
                    name,
                })
            },
        )?;

        // Select pragma_index_list
        let mut indexes: Vec<TableSchemaIndex> = self.select_all(
            &format!("PRAGMA index_list({})", escape_sql_identifier(table_name)),
            &[],
            |row| {
                let name = row.get::<_, String>("name")?;
                Ok(TableSchemaIndex {
                    seq: row.get::<_, i64>("seq")?,
                    unique: row.get::<_, i64>("unique")?,
                    origin: row.get::<_, String>("origin")?,
                    partial: row.get::<_, i64>("partial")?,
                    schema: get_sql_column(
                        self.select_all(
                            "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = ?",
                            &[Literal::String(name.to_owned())],
                            |row| Ok((row.get::<_, String>(0).ok(),)),
                        )
                        .ok(),
                    ),
                    columns: None,
                    name,
                })
            },
        )?;

        // List indexes
        for index in &mut indexes {
            index.columns = Some(self.select_all(
                &format!("PRAGMA index_info({})", escape_sql_identifier(&index.name)),
                &[],
                |row| {
                    Ok(IndexColumn {
                        seqno: row.get::<_, i64>(0)?,
                        cid: row.get::<_, i64>(1)?,
                        name: row.get::<_, Option<String>>(2)?,
                    })
                },
            )?);
        }

        // Get the table schema
        let schema = get_sql_column(
            self.select_all(
                "SELECT sql FROM sqlite_schema WHERE name = ?",
                &[Literal::String(table_name.to_owned())],
                |row| Ok((row.get::<_, String>(0).ok(),)),
            )
            .ok(),
        )
        .unwrap_or_else(|| "".to_string());

        // List triggers
        let triggers: Vec<TableSchemaTriggers> = self.select_all(
            "SELECT name, sql FROM sqlite_schema WHERE tbl_name = ? AND type = 'trigger'",
            &[Literal::String(table_name.to_owned())],
            |row| {
                Ok(TableSchemaTriggers {
                    name: row.get::<_, String>(0)?,
                    sql: row.get::<_, String>(1)?,
                })
            },
        )?;

        Ok(TableSchema {
            name: table_name.to_string(),
            type_: table_type,
            schema,
            has_rowid_column,
            strict,
            columns,
            indexes,
            triggers,
        })
    }

    pub(crate) fn load_extensions(&self, extensions: &[&str]) -> std::result::Result<(), String> {
        for ext in extensions {
            unsafe {
                self.con
                    .load_extension(ext, None)
                    .or_else(|err| Err(format!("Failed to load the extension {ext:?}: {err}")))?;
            }
        }
        Ok(())
    }

    pub fn handle(
        &self,
        mut w: &mut dyn Write,
        query: &str,
        params: &[Literal],
        mode: QueryMode,
    ) -> std::result::Result<(), QueryError> {
        let start_time = std::time::Instant::now();

        #[derive(Serialize, Debug, Clone)]
        struct EditorPragmaResponse<T: Serialize> {
            data: T,
            time: f64,
        }

        fn write_editor_pragma<T: Serialize>(
            w: &mut (impl Write + ?Sized),
            data: T,
            start_time: std::time::Instant,
        ) -> () {
            w.write_all(
                &rmp_serde::to_vec_named(&EditorPragmaResponse {
                    data,
                    time: start_time.elapsed().as_secs_f64(),
                })
                .expect("Failed to write msgpack"),
            )
            .expect("Failed to write msgpack");
        }

        match query {
            "EDITOR_PRAGMA database_label" => write_editor_pragma(w, self.database_label(), start_time),
            "EDITOR_PRAGMA list_tables" => write_editor_pragma(w, self.list_tables()?, start_time),
            "EDITOR_PRAGMA list_foreign_keys" => write_editor_pragma(w, self.list_foreign_keys()?, start_time),
            "EDITOR_PRAGMA table_schema" => {
                let Some(Literal::String(table_name)) = params.get(0) else {
                    return QueryError::new("invalid argument for `EDITOR_PRAGMA table_schema`", query, params);
                };
                write_editor_pragma(w, self.table_schema(table_name)?, start_time)
            }
            "EDITOR_PRAGMA load_extensions" => {
                let mut extensions = vec![];
                for param in params {
                    if let Literal::String(param) = param {
                        extensions.push(param.as_str());
                    }
                }
                write_editor_pragma(
                    w,
                    self.load_extensions(&extensions)
                        .or_else(|err| QueryError::new(err, query, params))?,
                    start_time,
                )
            }

            _ => {
                if mode == QueryMode::ReadOnly && NON_READONLY_SQL_PATTERN.is_match(query) {
                    return QueryError::new(
                        format!("Cannot execute {query:?} while in read-only mode."),
                        query,
                        params,
                    );
                }

                rmp::encode::write_map_len(&mut w, 2).expect("Failed to write msgpack");

                // Write records
                rmp::encode::write_str(&mut w, "records").expect("Failed to write msgpack");
                match mode {
                    QueryMode::Script => {
                        assert!(params.is_empty());
                        self.con
                            .execute_batch(query)
                            .or_else(|err| QueryError::new(err, query, params))?;
                        write_value_ref_into_msgpack(&mut w, ValueRef::Null).expect("Failed to write msgpack");
                    }
                    QueryMode::ReadOnly | QueryMode::ReadWrite => {
                        w.write_all(&self.execute(query, params)?)
                            .expect("Failed to write msgpack");
                    }
                }

                // Write time
                rmp::encode::write_str(&mut w, "time").expect("Failed to write msgpack");
                rmp::encode::write_f64(&mut w, start_time.elapsed().as_secs_f64()).expect("Failed to write msgpack");
            }
        }

        Ok(())
    }
}

impl Drop for SQLite3Driver {
    fn drop(&mut self) {
        if let Ok(mut stmt) = self.con.prepare("SELECT * FROM sqlite_schema LIMIT 1") {
            let _ = stmt.query(());
        }
        unsafe {
            let _ = ManuallyDrop::take(&mut self.con);
        };
    }
}

pub(crate) fn escape_sql_identifier(ident: &str) -> String {
    if ident.contains("\x00") {
        panic!("Failed to quote the SQL identifier {ident:?} as it contains a NULL char");
    }
    format!("\"{}\"", ident.replace("\"", "\"\""))
}

pub(crate) fn is_sqlcipher(con: &rusqlite::Connection) -> bool {
    con.pragma_query_value(None, "cipher_version", |row| match row.get_ref(0) {
        Ok(ValueRef::Text(_)) => Ok(true),
        _ => Ok(false),
    }) == Ok(true)
}

pub(crate) fn set_sqlcipher_key(con: &rusqlite::Connection, key: &str) -> std::result::Result<(), String> {
    if is_sqlcipher(&con) {
        con.pragma_update(None, "key", key)
            .expect("Setting `PRAGMA key` failed.");
        Ok(())
    } else {
        match std::env::current_exe() {
            Ok(p) => Err(format!("{} is not compiled with sqlcipher.", p.to_string_lossy())),
            _ => Err("This executable is not compiled with sqlcipher.".to_owned()),
        }
    }
}
