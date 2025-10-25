use std::time::Duration;

use rusqlite::functions::FunctionFlags;

pub fn add_sleep_fn(conn: &rusqlite::Connection) {
    let handle = SendSqliteHandle(unsafe { conn.handle() });
    conn.create_scalar_function("sleep", 1, FunctionFlags::SQLITE_UTF8, move |ms| {
        let total_duration = Duration::from_millis(ms.get::<i64>(0)? as u64);
        let start = std::time::Instant::now();
        while start.elapsed() < total_duration {
            std::thread::sleep(Duration::from_millis(1));

            // Support sqlite3_interrupt()
            if handle.is_interrupted() {
                return Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_INTERRUPT),
                    Some("interrupted".to_owned()),
                ));
            }
        }
        Ok(0)
    })
    .unwrap();
}

struct SendSqliteHandle(*mut rusqlite::ffi::sqlite3);

impl SendSqliteHandle {
    fn is_interrupted(&self) -> bool {
        unsafe { rusqlite::ffi::sqlite3_is_interrupted(self.0) != 0 }
    }
}

unsafe impl Send for SendSqliteHandle {}
