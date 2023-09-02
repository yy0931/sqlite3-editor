use rusqlite::ffi::{
    sqlite3, sqlite3_close, sqlite3_column_count, sqlite3_column_database_name, sqlite3_column_origin_name,
    sqlite3_column_table_name, sqlite3_errmsg, sqlite3_finalize, sqlite3_open, sqlite3_prepare_v2, sqlite3_stmt,
};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_int;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnOrigin {
    pub database: Option<String>,
    pub table: Option<String>,
    pub column: Option<String>,
}

pub fn column_origin(database: &str, query: &str) -> Result<Vec<ColumnOrigin>, String> {
    // Open the database connection
    let mut db: *mut sqlite3 = std::ptr::null_mut();
    let filename = CString::new(database).unwrap();
    let rc = unsafe { sqlite3_open(filename.as_ptr(), &mut db) };
    if rc != 0 {
        let msg = format!("Error opening database: {}", unsafe {
            CStr::from_ptr(sqlite3_errmsg(db)).to_string_lossy()
        });
        unsafe {
            sqlite3_close(db);
        }
        return Err(msg);
    }

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
        unsafe {
            sqlite3_close(db);
        }
        return Err(msg);
    }

    // Get the data sources and concatenate them using \0 as delimiters
    let mut result = vec![];

    let column_count = unsafe { sqlite3_column_count(stmt) as usize };
    for i in 0..column_count {
        let mut origin = ColumnOrigin::default();
        let str = unsafe { sqlite3_column_database_name(stmt, i as c_int) };
        if !str.is_null() {
            origin.database = unsafe { CStr::from_ptr(str) }.to_str().ok().map(|v| v.to_owned());
        }
        let str = unsafe { sqlite3_column_table_name(stmt, i as c_int) };
        if !str.is_null() {
            origin.table = unsafe { CStr::from_ptr(str) }.to_str().ok().map(|v| v.to_owned());
        }
        let str = unsafe { sqlite3_column_origin_name(stmt, i as c_int) };
        if !str.is_null() {
            origin.column = unsafe { CStr::from_ptr(str) }.to_str().ok().map(|v| v.to_owned());
        }
        result.push(origin);
    }

    // Finalize the statement and close the database connection
    unsafe {
        sqlite3_finalize(stmt);
        sqlite3_close(db);
    }

    Ok(result)
}
