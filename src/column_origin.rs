use rusqlite::ffi::{
    sqlite3, sqlite3_column_count, sqlite3_column_database_name, sqlite3_column_name, sqlite3_column_origin_name,
    sqlite3_column_table_name, sqlite3_errmsg, sqlite3_finalize, sqlite3_prepare_v2, sqlite3_stmt,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_int;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnOrigin {
    pub database: String,
    pub table: String,
    pub column: String,
}

impl ColumnOrigin {
    #[allow(dead_code)]
    pub fn new<T: Into<String>, U: Into<String>, V: Into<String>>(database: T, table: U, column: V) -> Self {
        Self {
            database: database.into(),
            table: table.into(),
            column: column.into(),
        }
    }
}

fn ptr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(ptr) }.to_str().ok().map(|v| v.to_owned())
    }
}

pub fn column_origin(db: *mut sqlite3, query: &str) -> Result<HashMap<String, ColumnOrigin>, String> {
    // Prepare the SELECT statement
    let mut stmt: *mut sqlite3_stmt = std::ptr::null_mut();
    let sql_query = CString::new(query).unwrap();
    let rc = unsafe { sqlite3_prepare_v2(db, sql_query.as_ptr(), -1, &mut stmt, std::ptr::null_mut()) };
    if rc != 0 {
        let msg = unsafe {
            format!(
                "Error preparing statement: {}",
                CStr::from_ptr(sqlite3_errmsg(db)).to_string_lossy()
            )
        };
        return Err(msg);
    }

    let mut result = HashMap::<String, ColumnOrigin>::new();

    let column_count = unsafe { sqlite3_column_count(stmt) as usize };
    for i in 0..column_count {
        let Some(column_name) = ptr_to_string(unsafe { sqlite3_column_name(stmt, i as i32) }) else {
            continue;
        };
        let (Some(database), Some(table), Some(column)) = (
            ptr_to_string(unsafe { sqlite3_column_database_name(stmt, i as c_int) }),
            ptr_to_string(unsafe { sqlite3_column_table_name(stmt, i as c_int) }),
            ptr_to_string(unsafe { sqlite3_column_origin_name(stmt, i as c_int) }),
        ) else {
            continue;
        };
        if table.to_lowercase().starts_with("pragma_") {
            continue;
        }
        result.insert(
            column_name,
            ColumnOrigin {
                database,
                table,
                column,
            },
        );
    }

    // Finalize the statement and close the database connection
    unsafe {
        sqlite3_finalize(stmt);
    }

    Ok(result)
}
