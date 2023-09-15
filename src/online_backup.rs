use std::ffi::{CStr, CString};

use rusqlite::ffi::{
    code_to_str, sqlite3, sqlite3_backup, sqlite3_backup_finish, sqlite3_backup_init, sqlite3_backup_pagecount,
    sqlite3_backup_remaining, sqlite3_backup_step, sqlite3_errmsg, sqlite3_sleep, SQLITE_BUSY, SQLITE_DONE,
    SQLITE_LOCKED, SQLITE_OK,
};

fn sqlite3_errmsg_as_string(handle: *mut sqlite3) -> String {
    unsafe { CStr::from_ptr(sqlite3_errmsg(handle)) }
        .to_string_lossy()
        .to_string()
}

#[derive(Debug, Clone)]
pub struct OnlineBackup {
    p_backup: *mut sqlite3_backup,
    done: bool,
    sleep: bool,
}

impl OnlineBackup {
    #[allow(unused)]
    pub fn new(src: *mut sqlite3, dst: *mut sqlite3, sleep: bool) -> std::result::Result<Self, String> {
        let main = CString::new("main").unwrap();

        let p_backup = unsafe { sqlite3_backup_init(dst, main.as_ptr(), src, main.as_ptr()) };
        if p_backup.is_null() {
            Err(sqlite3_errmsg_as_string(dst))
        } else {
            Ok(Self {
                p_backup,
                done: false,
                sleep,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub locked_or_busy: Option<String>,
    pub remaining: i64,
    pub pagecount: i64,
}

impl Iterator for OnlineBackup {
    type Item = Result<Step, String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        // Copy 100 pages
        let t = std::time::Instant::now();
        let rc = unsafe { sqlite3_backup_step(self.p_backup, 100) };
        let elapsed = t.elapsed();

        let step = Step {
            locked_or_busy: None,
            remaining: unsafe { sqlite3_backup_remaining(self.p_backup) } as i64,
            pagecount: unsafe { sqlite3_backup_pagecount(self.p_backup) } as i64,
        };

        match rc {
            SQLITE_BUSY | SQLITE_LOCKED => {
                unsafe { sqlite3_sleep(250) };
                Some(Ok(Step {
                    locked_or_busy: Some(code_to_str(rc).to_owned()),
                    ..step
                }))
            }
            SQLITE_OK => {
                if self.sleep {
                    // Wait 10 * (elapsed time)
                    unsafe { sqlite3_sleep((elapsed.as_millis() as std::os::raw::c_int) * 10) };
                }
                Some(Ok(step))
            }
            SQLITE_DONE => {
                self.done = true;
                None
            }
            _ => {
                self.done = true;
                Some(Err(code_to_str(rc).to_owned()))
            }
        }
    }
}

impl Drop for OnlineBackup {
    fn drop(&mut self) {
        unsafe { sqlite3_backup_finish(self.p_backup) };
    }
}
