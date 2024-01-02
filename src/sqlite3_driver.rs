use crate::{
    cache::{Pager, Records},
    column_origin::{column_origin, ColumnOrigin},
    find::{
        find_widget_compare, find_widget_compare_c, find_widget_compare_r, find_widget_compare_r_c,
        find_widget_compare_r_w, find_widget_compare_r_w_c, find_widget_compare_w, find_widget_compare_w_c,
    },
    literal::Literal,
    request_type::QueryMode,
};
use lazy_static::lazy_static;
use rusqlite::{functions::FunctionFlags, types::ValueRef, Row};
use serde::{Deserialize, Serialize};

use super::error::Error;
use std::{
    collections::HashMap,
    io::Write,
    mem::ManuallyDrop,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

#[derive(Debug, Clone)]
struct StringError(String);

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl std::error::Error for StringError {}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct InvalidUTF8 {
    pub text_lossy: String,
    pub bytes: String,
    pub context: Option<String>,
}

impl InvalidUTF8 {
    pub fn with(self, context: &str) -> Self {
        Self {
            context: Some(context.to_owned()),
            ..self
        }
    }
}

pub fn from_utf8_lossy<F: FnMut(InvalidUTF8)>(t: &[u8], mut on_invalid_utf8: F) -> String {
    match String::from_utf8(t.to_vec()) {
        Ok(s) => s,
        Err(_) => {
            let text_lossy = String::from_utf8_lossy(t).to_string();
            on_invalid_utf8(InvalidUTF8 {
                text_lossy: text_lossy.clone(),
                bytes: hex::encode(t),
                context: None,
            });
            text_lossy
        }
    }
}

pub fn get_string<F: FnMut(InvalidUTF8)>(row: &Row, idx: usize, on_invalid_utf8: F) -> rusqlite::Result<String> {
    let value = row.get_ref(idx)?;
    match value {
        ValueRef::Text(t) => Ok(from_utf8_lossy(t, on_invalid_utf8)),
        value => Err(rusqlite::Error::FromSqlConversionFailure(
            idx,
            value.data_type(),
            Box::new(StringError(format!("Expected Text but got {:?}.", value))),
        )),
    }
}

pub fn get_option_string<F: FnMut(InvalidUTF8)>(
    row: &Row,
    idx: usize,
    on_invalid_utf8: F,
) -> rusqlite::Result<Option<String>> {
    let value = row.get_ref(idx)?;
    match value {
        ValueRef::Null => Ok(None),
        ValueRef::Text(t) => Ok(Some(from_utf8_lossy(t, on_invalid_utf8))),
        value => Err(rusqlite::Error::FromSqlConversionFailure(
            idx,
            value.data_type(),
            Box::new(StringError(format!("Expected Text but got {:?}.", value))),
        )),
    }
}

pub struct SQLite3Driver {
    con: ManuallyDrop<rusqlite::Connection>,
    pager: Pager,
    abort_signal: Arc<AtomicBool>,
    pub database_label: String,
}

lazy_static! {
    static ref NON_READONLY_SQL_PATTERN: regex::Regex =
        regex::Regex::new(r"(?i)^\s*(INSERT|DELETE|UPDATE|CREATE|DROP|ALTER\s+TABLE)\b").unwrap();
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

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ts(export)]
pub struct TableSchemaColumnForeignKey {
    #[ts(type = "bigint")]
    pub id: i64,
    #[ts(type = "bigint")]
    pub seq: i64,
    pub table: String,
    /// Column name; foreign_key.to.unwrap_or(foreign_key.from)
    pub to: String,
    pub on_update: String,
    pub on_delete: String,
    #[serde(rename = "match")]
    pub match_: String,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ts(export)]
#[serde(untagged)]
pub enum DfltValue {
    Int(i64),
    Real(f64),
    String(String),
    Blob(#[ts(type = "Uint8Array")] Vec<u8>),
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ts(export)]
pub struct TableSchemaColumn {
    #[ts(type = "bigint")]
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
    pub foreign_keys: Rc<Vec<TableSchemaColumnForeignKey>>,
    /** 1: columns in virtual tables, 2: dynamic generated columns, 3: stored generated columns */
    #[ts(type = "0n | 1n | 2n | 3n")]
    pub hidden: i64,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct IndexColumn {
    #[ts(type = "bigint")]
    pub seqno: i64,
    #[ts(type = "bigint")]
    pub cid: i64,
    pub name: Option<String>,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct TableSchemaIndex {
    pub seq: i64,
    pub name: String,
    #[ts(type = "0n | 1n")]
    pub unique: i64,
    #[ts(type = "'c' | 'u' | 'pk'")]
    pub origin: String,
    #[ts(type = "0n | 1n")]
    pub partial: i64,
    pub columns: Vec<IndexColumn>,
    pub schema: Option<String>,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct TableSchemaTrigger {
    pub name: String,
    pub sql: String,
}

#[derive(ts_rs::TS, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct ColumnOriginAndIsRowId {
    pub database: String,
    pub table: String,
    pub column: String,
    pub is_rowid: bool,
}

impl ColumnOriginAndIsRowId {
    pub fn new(is_rowid: bool, column_origin: ColumnOrigin) -> Self {
        Self {
            is_rowid,
            database: column_origin.database,
            table: column_origin.table,
            column: column_origin.column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSchema {
    // InnerTableSchema
    pub schema: Option<String>,
    #[serde(rename = "hasRowIdColumn")]
    pub has_rowid_column: bool,
    pub strict: bool,
    pub columns: Vec<TableSchemaColumn>,
    pub indexes: Vec<TableSchemaIndex>,
    pub triggers: Vec<TableSchemaTrigger>,

    // Optional fields
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub type_: TableType,
    #[serde(rename = "customQuery")]
    pub custom_query: Option<String>,
    #[serde(rename = "columnOrigins")]
    pub column_origins: Option<HashMap<String, ColumnOriginAndIsRowId>>,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[ts(export)]
pub enum TableType {
    #[serde(rename = "table")]
    Table,
    #[serde(rename = "view")]
    View,
    #[serde(rename = "shadow")]
    Shadow,
    #[serde(rename = "virtual")]
    Virtual,

    /// query_schema() uses this
    #[serde(rename = "custom query")]
    CustomQuery,
    #[serde(rename = "other")]
    Other,
}

impl From<&str> for TableType {
    fn from(value: &str) -> Self {
        match value {
            "table" => Self::Table,
            "view" => Self::View,
            "virtual" => Self::Virtual,
            "shadow" => Self::Shadow,
            _ => Self::Other,
        }
    }
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[ts(export)]
pub struct Table {
    pub database: Rc<String>,
    pub name: Rc<String>,
    #[serde(rename = "type")]
    pub type_: TableType,
    pub column_names: Vec<String>,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct EntityRelationship {
    pub source: String,
    pub target: String,
    pub source_column: String,
    pub target_column: String,
}

pub fn write_value_ref_into_msgpack<W: rmp::encode::RmpWrite, F: FnMut(InvalidUTF8)>(
    wr: &mut W,
    value: ValueRef,
    on_invalid_utf8: F,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    match value {
        ValueRef::Null => {
            rmp::encode::write_nil(wr)?;
        }
        ValueRef::Integer(v) => {
            rmp::encode::write_sint(wr, v)?;
        }
        ValueRef::Text(v) => {
            rmp::encode::write_str(wr, &from_utf8_lossy(v, on_invalid_utf8))?;
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

type ForeignKeyList = HashMap</* column */ String, Rc<Vec<TableSchemaColumnForeignKey>>>;

pub fn connect_immutable(database_filepath: &str) -> std::result::Result<rusqlite::Connection, Error> {
    // Connect to the database with `?immutable=1` and the readonly flag
    const ASCII_SET: percent_encoding::AsciiSet = percent_encoding::NON_ALPHANUMERIC.remove(b'/');
    rusqlite::Connection::open_with_flags(
        format!(
            "file:{}?immutable=1",
            percent_encoding::utf8_percent_encode(database_filepath, &ASCII_SET)
        ),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .or_else(|err| Error::new_ffi_error(err, "sqlite3_open_v2 (immutable=1)", &[database_filepath.into()]))
}

fn assert_readonly_query(query: &str) -> std::result::Result<(), Error> {
    if NON_READONLY_SQL_PATTERN.is_match(query) {
        Error::new_other_error(
            "This query is not allowed in the read-only mode.",
            Some(query.to_owned()),
            None,
        )
    } else {
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExecMode {
    ReadOnly,
    ReadWrite,
}

#[derive(ts_rs::TS, Serialize, Debug, Clone)]
#[ts(export)]
struct EditorPragmaResponse<T: Serialize> {
    data: T,
    warnings: Vec<InvalidUTF8>,
    time: f64,
}

impl SQLite3Driver {
    /// Connects to the database, set busy_timeout to 500, register the find_widget_compare_r function, enable loading extensions, and fetch the version number of SQLite.
    /// * `read_only` - If true, connects to the database with immutable=1 and the readonly flag. Use this argument to read a database that is under an EXCLUSIVE lock.
    /// * `sql_cipher_key` - The encryption key for SQLCipher.
    pub fn connect(
        database_filepath: &str,
        read_only: bool,
        sql_cipher_key: &Option<impl AsRef<str>>,
    ) -> std::result::Result<Self, Error> {
        Self::connect_with_abort_signal(
            database_filepath,
            read_only,
            sql_cipher_key,
            Arc::new(AtomicBool::new(false)),
        )
    }

    pub fn connect_with_abort_signal(
        database_filepath: &str,
        read_only: bool,
        sql_cipher_key: &Option<impl AsRef<str>>,
        abort_signal: Arc<AtomicBool>,
    ) -> std::result::Result<Self, Error> {
        let con = if !read_only {
            // Connect to the database
            rusqlite::Connection::open(database_filepath)
                .or_else(|err| Error::new_ffi_error(err, "sqlite3_open_v2", &[database_filepath.into()]))?
        } else {
            connect_immutable(database_filepath)?
        };

        // Set the SQLite Cipher key if given
        if let Some(key) = sql_cipher_key {
            set_sqlcipher_key(&con, key.as_ref())?;
        }

        // Set busy_timeout to 500
        con.pragma_update(None, "busy_timeout", 500)
            .expect("Could not update the busy_timeout value");

        // Register functions
        con.create_scalar_function(
            "find_widget_compare_w_c",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare_w_c(ctx)),
        )
        .or_else(|err| Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare_w_c".into()]))?;

        con.create_scalar_function(
            "find_widget_compare_w",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare_w(ctx)),
        )
        .or_else(|err| Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare_w".into()]))?;

        con.create_scalar_function(
            "find_widget_compare_c",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare_c(ctx)),
        )
        .or_else(|err| Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare_c".into()]))?;

        con.create_scalar_function(
            "find_widget_compare",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare(ctx)),
        )
        .or_else(|err| Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare".into()]))?;

        con.create_scalar_function(
            "find_widget_compare_r_w_c",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare_r_w_c(ctx)),
        )
        .or_else(|err| {
            Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare_r_w_c".into()])
        })?;

        con.create_scalar_function(
            "find_widget_compare_r_w",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare_r_w(ctx)),
        )
        .or_else(|err| Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare_r_w".into()]))?;

        con.create_scalar_function(
            "find_widget_compare_r_c",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare_r_c(ctx)),
        )
        .or_else(|err| Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare_r_c".into()]))?;

        con.create_scalar_function(
            "find_widget_compare_r",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| Ok(find_widget_compare_r(ctx)),
        )
        .or_else(|err| Error::new_ffi_error(err, "sqlite3_create_function_v2", &["find_widget_compare_r".into()]))?;

        // Allow loading extensions
        unsafe {
            con.load_extension_enable()
                .expect("Failed to enable loading run-time loadable extensions.");
        }

        let database_label = format!(
            "{} {}",
            if is_sqlcipher(&con) { "sqlcipher" } else { "sqlite" },
            rusqlite::version()
        );

        // Get the SQLite version as a string
        Ok(Self {
            con: ManuallyDrop::new(con),
            pager: Pager::new(),
            abort_signal,
            database_label,
        })
    }

    pub fn abort_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.abort_signal)
    }

    #[cfg(test)]
    pub fn pager(&mut self) -> &mut Pager {
        &mut self.pager
    }

    /// Executes a SQL statement and returns the result as a msgpack.
    pub fn execute(
        &mut self,
        query: &str,
        params: &[Literal],
        read_only: ExecMode,
        warnings: &mut Vec<InvalidUTF8>,
    ) -> std::result::Result<Vec<u8>, Error> {
        if read_only == ExecMode::ReadOnly {
            assert_readonly_query(query)?;
        } else {
            self.pager.clear_cache();
        }

        let records = if let Some(records) = self
            .pager
            .query(&mut self.con, query, params, |err| warnings.push(err.with(query)))?
        {
            records
        } else {
            // Prepare
            let mut stmt = self
                .con
                .prepare(query)
                .or_else(|err| Error::new_query_error(err, query, params))?;

            // Bind parameters
            for (i, param) in params.iter().enumerate() {
                stmt.raw_bind_parameter(i + 1, param)
                    .or_else(|err| Error::new_query_error(err, query, params))?;
            }

            // List columns
            let columns = stmt
                .column_names()
                .into_iter()
                .map(|v| v.to_owned())
                .collect::<Vec<_>>();

            // Fetch records
            let mut col_buf: Vec<Vec<u8>> = vec![vec![]; columns.len()];

            let mut n_rows = 0;
            let mut rows = stmt.raw_query();
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        for (i, col_buf_i) in col_buf.iter_mut().enumerate() {
                            write_value_ref_into_msgpack(col_buf_i, row.get_ref_unwrap(i), |err| {
                                warnings.push(err.with(query))
                            })
                            .expect("Failed to write msgpack");
                        }
                        n_rows += 1;
                    }
                    Ok(None) => break,
                    Err(err) => Error::new_query_error(err, query, params)?,
                }
            }
            Records::new(col_buf, n_rows, Rc::new(columns))
        };

        // Pack the result into a msgpack
        let mut buf = vec![];
        rmp::encode::write_map_len(&mut buf, records.columns().len() as u32).expect("Failed to write msgpack");
        for (i, column_name) in records.columns().iter().enumerate() {
            rmp::encode::write_str(&mut buf, column_name).expect("Failed to write msgpack");
            rmp::encode::write_array_len(&mut buf, records.n_rows() as u32).expect("Failed to write msgpack");
            buf.extend(&records.col_buf()[i]);
        }
        Ok(buf)
    }

    /// Executes a SQL statement and returns the result.
    pub fn select_all<F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>, T>(
        &self,
        query: &str,
        params: &[Literal],
        map: F,
    ) -> std::result::Result<Vec<T>, Error> {
        assert_readonly_query(query)?;

        // Prepare the statement
        let mut stmt = self
            .con
            .prepare(query)
            .or_else(|err| Error::new_query_error(err, query, params))?;

        // Fetch data and pack into Vec<T>
        let mut col_buf: Vec<Vec<u8>> = vec![];
        let column_count = stmt.column_count();
        for _ in 0..column_count {
            col_buf.push(vec![]);
        }
        for (i, param) in params.iter().enumerate() {
            stmt.raw_bind_parameter(i + 1, param)
                .or_else(|err| Error::new_query_error(err, query, params))?;
        }
        let records = stmt
            .raw_query()
            .mapped::<F, T>(map)
            .collect::<rusqlite::Result<Vec<T>>>()
            .or_else(|err| Error::new_query_error(err, query, params))?;

        Ok(records)
    }

    pub fn list_tables(
        &self,
        include_system_tables: bool,
    ) -> std::result::Result<(Vec<Table>, Vec<InvalidUTF8>), Error> {
        let mut warnings = vec![];
        let mut tables = self.select_all(
            &(r#"SELECT schema, name, type FROM pragma_table_list"#.to_owned()
                + if include_system_tables {
                    ""
                } else {
                    r#" WHERE NOT (name LIKE "sqlite\_%" ESCAPE "\")"#
                }),
            &[],
            |row| {
                Ok(Table {
                    database: Rc::new(get_string(row, 0, |err| {
                        warnings.push(err.with("pragma_table_list.schema"))
                    })?),
                    name: Rc::new(get_string(row, 1, |err| {
                        warnings.push(err.with("pragma_table_list.name"))
                    })?),
                    type_: TableType::from(
                        get_string(row, 2, |err| {
                            warnings.push(err.with("pragma_table_list.type (list_tables)"))
                        })?
                        .as_str(),
                    ),
                    column_names: vec![],
                })
            },
        )?;

        let mut column_names_map = HashMap::<String, Vec<String>>::new();

        // Ignore broken tables
        let _ = self.select_all(
            r#"
WITH tables AS (
    SELECT DISTINCT name AS "table_name"
    FROM pragma_table_list()
    WHERE schema = 'main')
SELECT table_name, p.name
FROM tables
JOIN main.pragma_table_info("table_name") p"#,
            &[],
            |row| {
                column_names_map
                    .entry(get_string(row, 0, |err| {
                        warnings.push(err.with("list_columns.table_name"))
                    })?)
                    .or_default()
                    .push(get_string(row, 1, |err| {
                        warnings.push(err.with("list_columns.column_name"))
                    })?);
                Ok(0)
            },
        );

        for table in &mut tables {
            if let Some(columns) = column_names_map.remove(table.name.as_ref()) {
                for column in columns {
                    table.column_names.push(column);
                }
            }
        }

        Ok((tables, warnings))
    }

    pub fn list_entity_relationships(&self) -> std::result::Result<(Vec<EntityRelationship>, Vec<InvalidUTF8>), Error> {
        let mut warnings = vec![];

        // foreign keys
        let mut result = self.select_all(
            r#"SELECT t.name, f."table", f."from", f."to" FROM pragma_table_list t INNER JOIN pragma_foreign_key_list(name) f WHERE t.schema == 'main' AND NOT (t.name LIKE "sqlite\_%" ESCAPE "\");"#,
            &[],
            |row| {
                let source_column = get_string(row, 2, |err| warnings.push(err.with("pragma_table_list.from")))?;
                Ok(EntityRelationship {
                    source: get_string(row, 0, |err| warnings.push(err.with("pragma_table_list.name")))?,
                    target: get_string(row, 1, |err| warnings.push(err.with("pragma_table_list.table")))?,
                    target_column: get_option_string(row, 3, |err| warnings.push(err.with("pragma_table_list.to")))?.unwrap_or_else(|| source_column.clone()),
                    source_column,
                })
            },
        )?;

        // views
        if let Ok(views) = self.select_all(
            "SELECT name FROM pragma_table_list WHERE schema = 'main' AND type = 'view'",
            &[],
            |row| get_string(row, 0, |err| warnings.push(err.with("pragma_table_list.name"))),
        ) {
            for view_name in views {
                let Ok(column_origins) = column_origin(
                    unsafe { self.con.handle() },
                    &format!("SELECT * FROM {} LIMIT 0", escape_sql_identifier(&view_name)),
                ) else {
                    continue;
                };
                for (column, origin) in column_origins {
                    if origin.database.to_lowercase() != "main" {
                        continue;
                    }
                    result.push(EntityRelationship {
                        source: view_name.clone(),
                        target: origin.table,
                        source_column: column,
                        target_column: origin.column,
                    })
                }
            }
        }

        Ok((result, warnings))
    }

    /// Returns foreign keys for each column.
    fn foreign_keys<'a>(
        &self,
        database: &str,
        table_name: &str,
        warnings: &mut Vec<InvalidUTF8>,
        cache: &'a mut HashMap<(String, String), ForeignKeyList>,
    ) -> std::result::Result<&'a ForeignKeyList, Error> {
        let cache_key = (database.to_owned(), table_name.to_owned());

        if !cache.contains_key(&cache_key) {
            let mut result = HashMap::<String, Vec<TableSchemaColumnForeignKey>>::new();
            self.select_all(
                &format!(
                    "PRAGMA {}.foreign_key_list({})",
                    escape_sql_identifier(database),
                    escape_sql_identifier(table_name)
                ),
                &[],
                |row| {
                    let from = get_string(row, 3, |err| warnings.push(err.with("foreign_key_list.from")))?;
                    let to = get_option_string(row, 4, |err| warnings.push(err.with("foreign_key_list.to")))?;
                    result
                        .entry(from.clone())
                        .or_default()
                        .push(TableSchemaColumnForeignKey {
                            id: row.get::<_, i64>(0)?,
                            seq: row.get::<_, i64>(1)?,
                            table: get_string(row, 2, |err| warnings.push(err.with("foreign_key_list.table")))?,
                            to: to.unwrap_or(from),
                            on_update: get_string(row, 5, |err| warnings.push(err.with("foreign_key_list.on_update")))?,
                            on_delete: get_string(row, 6, |err| warnings.push(err.with("foreign_key_list.on_delete")))?,
                            match_: get_string(row, 7, |err| warnings.push(err.with("foreign_key_list.match")))?,
                        });
                    Ok(())
                },
            )?;

            let result = result
                .into_iter()
                .map(|(k, v)| (k, Rc::new(v)))
                .collect::<HashMap<_, _>>();
            cache.insert(cache_key.clone(), result);
        }

        Ok(cache.get(&cache_key).unwrap())
    }

    pub fn is_rowid(
        &self,
        column_origin: &ColumnOrigin,
        warnings: &mut Vec<InvalidUTF8>,
    ) -> std::result::Result<bool, Error> {
        struct Column {
            name: String,
            type_: String,
            pk: i64,
        }

        let table_xinfo = self.select_all(
            &format!(
                "PRAGMA {}.table_xinfo({})",
                escape_sql_identifier(&column_origin.database),
                escape_sql_identifier(&column_origin.table)
            ),
            &[],
            |row| {
                Ok(Column {
                    name: get_string(row, 1, |err| warnings.push(err.with("is_rowid.table_xinfo.name")))?,
                    type_: get_string(row, 2, |err| warnings.push(err.with("is_rowid.table_xinfo.type")))?,
                    pk: row.get::<_, i64>(5)?,
                })
            },
        )?;

        // - column_origin.column is "rowid" and there isn't a user-defined column named "rowid".
        // - column_origin.column is "_rowid_" and there isn't a user-defined column named "_rowid_".
        // - column_origin.column is "oid" and there isn't a user-defined column named "oid".
        if column_origin.column.to_lowercase() == "rowid"
            && table_xinfo.iter().all(|v| v.name.to_lowercase() != "rowid")
            || column_origin.column.to_lowercase() == "_rowid_"
                && table_xinfo.iter().all(|v| v.name.to_lowercase() != "_rowid_")
            || column_origin.column.to_lowercase() == "oid"
                && table_xinfo.iter().all(|v| v.name.to_lowercase() != "oid")
        {
            return Ok(true);
        }

        // - column_origin.column is a INTEGER PRIMARY KEY, where "INTEGER" need to be case-insensitive exact match, and there aren't multiple primary keys in the table.
        // > In the exception, the INTEGER PRIMARY KEY becomes an alias for the rowid.
        // > https://www.sqlite.org/rowidtable.html
        // sqlite3_column_origin_name() returns the INTEGER PRIMARY KEY column when queried against rowid.
        if table_xinfo
            .iter()
            .find(|v| v.name.to_lowercase() == column_origin.column.to_lowercase())
            .is_some_and(|v| v.type_.to_lowercase() == "integer" && v.pk != 0)
            && table_xinfo.iter().filter(|v| v.pk != 0).count() == 1
        {
            return Ok(true);
        }

        Ok(false)
    }

    /// Collect table definitions from sqlite_schema, pragma_table_list, pragma_foreign_key_list, pragma_table_xinfo, pragma_index_list, and pragm_index_info.
    pub fn table_schema(
        &self,
        database: &str,
        table_name: &str,
    ) -> std::result::Result<(Option<TableSchema>, Vec<InvalidUTF8>), Error> {
        let mut warnings = vec![];

        // Select pragma_table_list
        let (table_type, wr, strict) = {
            let all = self.select_all(
                "SELECT type, wr, strict FROM pragma_table_list WHERE schema = ? COLLATE NOCASE AND name = ? COLLATE NOCASE",
                &[database.into(), table_name.into()],
                |row| {
                    Ok((
                        get_string(row, 0, |err| {
                            warnings.push(err.with("pragma_table_list.type (table_schema)"))
                        })?,
                        row.get::<_, i64>(1)? != 0,
                        row.get::<_, i64>(2)? != 0,
                    ))
                },
            )?;
            let Some(one) = all.first() else {
                return Ok((None, warnings));
            };
            one.to_owned()
        };

        let table_type = TableType::from(table_type.as_str());
        let has_rowid_column = table_type == TableType::Table && !wr;

        // Select pragma_foreign_key_list
        let mut foreign_key_list_cache = HashMap::new();

        #[allow(clippy::type_complexity)]
        let (column_origins, foreign_keys): (
            Option<HashMap<String, ColumnOriginAndIsRowId>>,
            HashMap<String, Rc<Vec<TableSchemaColumnForeignKey>>>,
        ) = if table_type == TableType::View {
            let column_origins = column_origin(
                unsafe { self.con.handle() },
                &format!("SELECT * FROM {} LIMIT 0", escape_sql_identifier(table_name)),
            )
            .unwrap_or_default();

            // In this case:
            // ```
            // CREATE TABLE t1(x INTEGER PRIMARY KEY);
            // CREATE TABLE t2(y INTEGER REFERENCES t1(x));
            // CREATE VIEW table_name AS SELECT y as z FROM t2;
            // ```
            // column_origins = {"z": ("main", "t2", "y")}
            // origin_fk = { from: "y", to: "x", table: "t1" }
            let mut foreign_keys = HashMap::<String, Vec<TableSchemaColumnForeignKey>>::new();
            for (from, to) in &column_origins {
                if let Some(origin_fk) = self
                    .foreign_keys(&to.database, &to.table, &mut warnings, &mut foreign_key_list_cache)?
                    .get(&to.column)
                {
                    let vec = foreign_keys.entry(from.to_owned()).or_default();
                    for item in origin_fk.iter() {
                        vec.push(item.to_owned());
                    }
                }
            }
            (
                Some(
                    column_origins
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                k,
                                ColumnOriginAndIsRowId::new(
                                    self.is_rowid(&v, &mut warnings)
                                        .unwrap_or(false /* TODO: error handling */),
                                    v,
                                ),
                            )
                        })
                        .collect::<HashMap<String, ColumnOriginAndIsRowId>>(),
                ),
                foreign_keys.into_iter().map(|(k, v)| (k, Rc::new(v))).collect(),
            )
        } else {
            (
                None,
                self.foreign_keys(database, table_name, &mut warnings, &mut foreign_key_list_cache)?
                    .clone(),
            )
        };

        // Select sqlite_sequence
        // NOTE: There is no way to check if an empty table has an autoincrement column.
        let has_table_auto_increment_column: bool = !self
            .select_all(
                &format!(
                    "SELECT name FROM {}.sqlite_schema WHERE type = 'table' AND name = 'sqlite_sequence'",
                    escape_sql_identifier(database)
                ),
                &[],
                |_row| Ok(()),
            )?
            .is_empty()
            && !self
                .select_all(
                    &format!(
                        "SELECT * FROM {}.sqlite_sequence WHERE name = ? COLLATE NOCASE",
                        escape_sql_identifier(database)
                    ),
                    &[table_name.into()],
                    |_row| Ok(()),
                )?
                .is_empty();

        let get_sql_column = |records: Option<Vec<(Option<std::string::String>,)>>| -> Option<String> {
            if let Some(mut records) = records {
                if !records.is_empty() {
                    return std::mem::take(&mut records[0].0);
                }
            }
            None
        };

        // Select pragma_table_xinfo
        let columns: Vec<TableSchemaColumn> = self.select_all(
            &format!(
                "PRAGMA {}.table_xinfo({})",
                escape_sql_identifier(database),
                escape_sql_identifier(table_name)
            ),
            &[],
            |row| {
                let name = get_string(row, 1, |err| warnings.push(err.with("table_xinfo.name")))?;
                let pk = row.get::<_, i64>(5)? != 0;

                Ok(TableSchemaColumn {
                    cid: row.get::<_, i64>(0)?,
                    notnull: row.get::<_, i64>(3)? != 0,
                    type_: if table_type == TableType::View {
                        // NOTE: Why does table_xinfo always return "BLOB" for views?
                        "".to_owned()
                    } else {
                        get_string(row, 2, |err| warnings.push(err.with("table_xinfo.type")))?
                    },
                    pk,
                    auto_increment: pk && has_table_auto_increment_column,
                    foreign_keys: foreign_keys.get(&name).map(Rc::clone).unwrap_or_default(),
                    hidden: row.get::<_, i64>(6)?,
                    dflt_value: match row.get_ref(4)? {
                        ValueRef::Null => None,
                        ValueRef::Integer(v) => Some(DfltValue::Int(v)),
                        ValueRef::Real(v) => Some(DfltValue::Real(v)),
                        ValueRef::Text(v) => Some(DfltValue::String(from_utf8_lossy(v, |err| {
                            warnings.push(err.with("table_xinfo.dflt_value"))
                        }))),
                        ValueRef::Blob(v) => Some(DfltValue::Blob(v.to_owned())),
                    },
                    name,
                })
            },
        )?;

        // Select pragma_index_list
        let mut indexes: Vec<TableSchemaIndex> = self.select_all(
            &format!(
                "PRAGMA {}.index_list({})",
                escape_sql_identifier(database),
                escape_sql_identifier(table_name)
            ),
            &[],
            |row| {
                let name = get_string(row, 1, |err| warnings.push(err.with("index_list.name")))?;
                Ok(TableSchemaIndex {
                    seq: row.get::<_, i64>(0)?,
                    unique: row.get::<_, i64>(2)?,
                    origin: get_string(row, 3, |err| warnings.push(err.with("index_list.origin")))?,
                    partial: row.get::<_, i64>(4)?,
                    schema: get_sql_column(
                        self.select_all(
                            &format!(
                                "SELECT sql FROM {}.sqlite_schema WHERE type = 'index' AND name = ? COLLATE NOCASE",
                                escape_sql_identifier(database)
                            ),
                            &[Literal::String(name.to_owned())],
                            |row| Ok((get_string(row, 0, |err| warnings.push(err.with("sqlite_schema.sql"))).ok(),)),
                        )
                        .ok(),
                    ),
                    columns: vec![], // this will be replaced
                    name,
                })
            },
        )?;

        // List indexes
        for index in &mut indexes {
            index.columns = self.select_all(
                &format!("PRAGMA index_info({})", escape_sql_identifier(&index.name)),
                &[],
                |row| {
                    Ok(IndexColumn {
                        seqno: row.get::<_, i64>(0)?,
                        cid: row.get::<_, i64>(1)?,
                        name: get_option_string(row, 2, |err| warnings.push(err.with("index_info.name")))?,
                    })
                },
            )?;
        }

        // Get the table schema
        let schema = get_sql_column(
            self.select_all(
                &format!(
                    "SELECT sql FROM {}.sqlite_schema WHERE name = ? COLLATE NOCASE",
                    escape_sql_identifier(database)
                ),
                &[table_name.into()],
                |row| Ok((get_string(row, 0, |err| warnings.push(err.with("sqlite_schema.sql (table)"))).ok(),)),
            )
            .ok(),
        )
        .unwrap_or_else(|| "".to_string());

        // List triggers
        let triggers: Vec<TableSchemaTrigger> = self.select_all(
            &format!(
                "SELECT name, sql FROM {}.sqlite_schema WHERE tbl_name = ? AND type = 'trigger'",
                escape_sql_identifier(database)
            ),
            &[table_name.into()],
            |row| {
                Ok(TableSchemaTrigger {
                    name: get_string(row, 0, |err| warnings.push(err.with("sqlite_schema.name (trigger)")))?,
                    sql: get_string(row, 1, |err| warnings.push(err.with("sqlite_schema.sql (trigger)")))?,
                })
            },
        )?;

        Ok((
            Some(TableSchema {
                name: Some(table_name.to_string()),
                schema: Some(schema),
                has_rowid_column,
                strict,
                columns,
                indexes,
                triggers,
                custom_query: None,
                column_origins,
                type_: table_type,
            }),
            warnings,
        ))
    }

    pub fn query_schema(&self, query: &str) -> std::result::Result<(TableSchema, Vec<InvalidUTF8>), Error> {
        let mut warnings = vec![];

        let column_origins = column_origin(
            unsafe { self.con.handle() },
            // \n is to handle comments, e.g. customQuery = "SELECT ... FROM ... -- comments"
            &format!("SELECT * FROM ({query}\n) LIMIT 0"),
        )
        .unwrap_or_default();

        let stmt = format!("SELECT * FROM ({query}\n) LIMIT 0");
        let column_names = self
            .con
            .prepare(&stmt)
            .or_else(|err| Error::new_query_error(err, &stmt, &[]))?
            .column_names()
            .into_iter()
            .map(|v| v.to_owned())
            .collect::<Vec<_>>();

        let mut foreign_key_list_cache = HashMap::new();

        Ok((
            TableSchema {
                type_: TableType::CustomQuery,
                name: None,
                indexes: vec![],
                triggers: vec![],
                schema: None,
                has_rowid_column: false,
                strict: false,
                columns: column_names
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| TableSchemaColumn {
                        cid: i as i64,
                        dflt_value: None,
                        name: name.to_owned(),
                        notnull: false,
                        type_: "".to_owned(),
                        pk: false,
                        auto_increment: false,
                        foreign_keys: column_origins
                            .get(&name)
                            .and_then(|origin| {
                                self.foreign_keys(
                                    &origin.database,
                                    &origin.table,
                                    &mut warnings,
                                    &mut foreign_key_list_cache,
                                )
                                .ok()
                                .and_then(|map| map.get(&origin.column).cloned())
                            })
                            .unwrap_or_default(),
                        hidden: 0,
                    })
                    .collect::<Vec<_>>(),
                custom_query: Some(query.to_owned()),
                column_origins: Some(
                    column_origins
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                k,
                                ColumnOriginAndIsRowId::new(
                                    self.is_rowid(&v, &mut warnings)
                                        .unwrap_or(false /* TODO: error handling */),
                                    v,
                                ),
                            )
                        })
                        .collect::<HashMap<String, ColumnOriginAndIsRowId>>(),
                ),
            },
            warnings,
        ))
    }

    pub fn load_extensions(&self, extensions: &[&str]) -> std::result::Result<(), Error> {
        for ext in extensions {
            unsafe {
                self.con.load_extension(ext, None).or_else(|err| {
                    Error::new_ffi_error(
                        err,
                        "sqlite3_load_extension",
                        &extensions.iter().map(|v| (*v).into()).collect::<Vec<Literal>>(),
                    )
                })?;
            }
        }
        Ok(())
    }

    pub fn handle(
        &mut self,
        mut w: &mut dyn Write,
        query: &str,
        params: &[Literal],
        mode: QueryMode,
    ) -> std::result::Result<(), Error> {
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
            "EDITOR_PRAGMA list_tables" => write_editor_pragma(w, self.list_tables(false)?, start_time),
            "EDITOR_PRAGMA list_entity_relationships" => {
                write_editor_pragma(w, self.list_entity_relationships()?, start_time)
            }
            "EDITOR_PRAGMA table_schema" => {
                let (Some(Literal::String(database)), Some(Literal::String(table_name))) =
                    (params.first(), params.get(1))
                else {
                    return Error::new_other_error(
                        "invalid arguments for `EDITOR_PRAGMA table_schema`",
                        Some(query.to_owned()),
                        Some(params),
                    );
                };
                write_editor_pragma(w, self.table_schema(database, table_name)?, start_time)
            }
            "EDITOR_PRAGMA query_schema" => {
                let Some(Literal::String(query)) = params.first() else {
                    return Error::new_other_error(
                        "invalid argument for `EDITOR_PRAGMA query_schema`",
                        Some(query.to_owned()),
                        Some(params),
                    );
                };
                write_editor_pragma(w, self.query_schema(query)?, start_time)
            }
            "EDITOR_PRAGMA total_cache_size_bytes" => {
                write_editor_pragma(w, (self.pager.total_cache_size_bytes(), vec![]), start_time)
            }
            "EDITOR_PRAGMA load_extensions" => {
                let mut extensions = vec![];
                for param in params {
                    if let Literal::String(param) = param {
                        extensions.push(param.as_str());
                    }
                }
                write_editor_pragma(w, (self.load_extensions(&extensions)?, vec![]), start_time)
            }
            "EDITOR_PRAGMA add_sleep_fn" => {
                self.con
                    .create_scalar_function("sleep", 1, FunctionFlags::SQLITE_UTF8, |ms| {
                        std::thread::sleep(Duration::from_millis(ms.get(0)?));
                        Ok(0)
                    })
                    .unwrap();
                write_editor_pragma(w, (0, vec![]), start_time)
            }

            _ => {
                if mode == QueryMode::ReadOnly {
                    assert_readonly_query(query)?;
                }

                rmp::encode::write_map_len(&mut w, 3).expect("Failed to write msgpack");

                let mut warnings = vec![];

                // Write records
                rmp::encode::write_str(&mut w, "records").expect("Failed to write msgpack");
                match mode {
                    QueryMode::Script => {
                        assert!(params.is_empty());
                        self.pager.clear_cache();
                        let result = self.con.execute_batch(query);

                        // Rollback uncommitted transactions
                        let _ = self.con.execute("ROLLBACK;", ());

                        result.or_else(|err| Error::new_query_error(err, query, params))?;

                        write_value_ref_into_msgpack(&mut w, ValueRef::Null, |err| {
                            warnings.push(err.with("write null"))
                        })
                        .expect("Failed to write msgpack");
                    }
                    QueryMode::ReadOnly => {
                        w.write_all(&self.execute(query, params, ExecMode::ReadOnly, &mut warnings)?)
                            .expect("Failed to write msgpack");
                    }
                    QueryMode::ReadWrite => {
                        w.write_all(&self.execute(query, params, ExecMode::ReadWrite, &mut warnings)?)
                            .expect("Failed to write msgpack");
                    }
                }

                // Write warnings
                rmp::encode::write_str(&mut w, "warnings").expect("Failed to write msgpack");
                w.write_all(&rmp_serde::to_vec_named(&warnings).unwrap()).unwrap();

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

pub fn escape_sql_identifier(ident: &str) -> String {
    if ident.contains('\x00') {
        panic!("Failed to quote the SQL identifier {ident:?} as it contains a NULL char");
    }
    format!("\"{}\"", ident.replace('"', "\"\""))
}

pub fn is_sqlcipher(con: &rusqlite::Connection) -> bool {
    con.pragma_query_value(None, "cipher_version", |row| match row.get_ref(0) {
        Ok(ValueRef::Text(_)) => Ok(true),
        _ => Ok(false),
    }) == Ok(true)
}

pub fn set_sqlcipher_key(con: &rusqlite::Connection, key: &str) -> std::result::Result<(), Error> {
    if is_sqlcipher(con) {
        con.pragma_update(None, "key", key)
            .expect("Setting `PRAGMA key` failed.");
        Ok(())
    } else {
        match std::env::current_exe() {
            Ok(p) => Error::new_other_error(
                format!("{} is not compiled with sqlcipher.", p.to_string_lossy()),
                None,
                None,
            ),
            _ => Error::new_other_error("This executable is not compiled with sqlcipher.", None, None),
        }
    }
}

pub fn read_msgpack_into_json(mut r: impl std::io::Read + std::io::Seek) -> String {
    r.rewind().expect("Failed to rewind the reader.");
    let mut json = vec![];
    match serde_transcode::transcode(
        &mut rmp_serde::Deserializer::new(&mut r),
        &mut serde_json::Serializer::new(&mut json),
    ) {
        Ok(_) => String::from_utf8_lossy(&json).to_string(),
        Err(_) => "<Failed to serialize as a JSON>".to_owned(),
    }
}
