use rusqlite::ffi::sqlite3;
use rusqlite::ffi::sqlite3_column_count;
use rusqlite::ffi::sqlite3_column_database_name;
use rusqlite::ffi::sqlite3_column_name;
use rusqlite::ffi::sqlite3_column_origin_name;
use rusqlite::ffi::sqlite3_column_table_name;
use rusqlite::ffi::sqlite3_errmsg;
use rusqlite::ffi::sqlite3_finalize;
use rusqlite::ffi::sqlite3_prepare_v2;
use rusqlite::ffi::sqlite3_step;
use rusqlite::ffi::sqlite3_stmt;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::c_char;
use std::ffi::CStr;
use std::ffi::CString;

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, derive_new::new)]
pub struct ColumnOrigin {
    #[new(into)]
    pub database: String,
    #[new(into)]
    pub table: String,
    #[new(into)]
    pub column: String,
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

    // NOTE: We need to call `sqlite3_column_count()` and `sqlite3_column_name()` after `sqlite3_step()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53)
    unsafe { sqlite3_step(stmt) };

    let column_count: usize = unsafe { sqlite3_column_count(stmt).try_into().unwrap() };
    for i in 0..column_count {
        let Some(column_name) = ptr_to_string(unsafe { sqlite3_column_name(stmt, i.try_into().unwrap()) }) else {
            continue;
        };
        let (Some(database), Some(table), Some(column)) = (
            ptr_to_string(unsafe { sqlite3_column_database_name(stmt, i.try_into().unwrap()) }),
            ptr_to_string(unsafe { sqlite3_column_table_name(stmt, i.try_into().unwrap()) }),
            ptr_to_string(unsafe { sqlite3_column_origin_name(stmt, i.try_into().unwrap()) }),
        ) else {
            continue;
        };
        if table.to_lowercase().starts_with("pragma_") {
            continue;
        }
        result.insert(column_name, ColumnOrigin { database, table, column });
    }

    // Finalize the statement and close the database connection
    unsafe {
        sqlite3_finalize(stmt);
    }

    Ok(result)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::column_origin;
    use super::ColumnOrigin;

    #[test]
    fn test_simple() {
        let con = rusqlite::Connection::open_in_memory().unwrap();

        con.execute_batch(
            "
CREATE TABLE t1(c1 TEXT NOT NULL, c2 TEXT NOT NULL);
CREATE VIEW v1 AS SELECT c1 as c3, c2 FROM t1;
",
        )
        .unwrap();

        assert_eq!(
            column_origin(unsafe { con.handle() }, "SELECT * FROM v1"),
            Ok(HashMap::from([
                ("c3".to_owned(), ColumnOrigin::new("main", "t1", "c1")),
                ("c2".to_owned(), ColumnOrigin::new("main", "t1", "c2")),
            ]))
        );

        assert_eq!(column_origin(unsafe { con.handle() }, "SELECT 1, 2"), Ok(HashMap::new()),);
    }

    #[test]
    fn test_sqlite_prepare_error() {
        let con = rusqlite::Connection::open_in_memory().unwrap();
        assert_eq!(
            column_origin(unsafe { con.handle() }, "SELEC"),
            Err("Error preparing statement: near \"SELEC\": syntax error".to_owned())
        );
    }
}
