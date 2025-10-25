//! This module is not currently used.

use std::ffi::CStr;
use std::ffi::CString;

use rusqlite::ffi::code_to_str;
use rusqlite::ffi::sqlite3;
use rusqlite::ffi::sqlite3_backup;
use rusqlite::ffi::sqlite3_backup_finish;
use rusqlite::ffi::sqlite3_backup_init;
use rusqlite::ffi::sqlite3_backup_pagecount;
use rusqlite::ffi::sqlite3_backup_remaining;
use rusqlite::ffi::sqlite3_backup_step;
use rusqlite::ffi::sqlite3_errmsg;
use rusqlite::ffi::sqlite3_sleep;
use rusqlite::ffi::SQLITE_BUSY;
use rusqlite::ffi::SQLITE_DONE;
use rusqlite::ffi::SQLITE_LOCKED;
use rusqlite::ffi::SQLITE_OK;

#[derive(Clone, Debug)]
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

fn sqlite3_errmsg_as_string(handle: *mut sqlite3) -> String {
    unsafe { CStr::from_ptr(sqlite3_errmsg(handle)) }
        .to_string_lossy()
        .to_string()
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[cfg(test)]
mod test {
    use tempfile::NamedTempFile;

    use super::OnlineBackup;

    #[test]
    fn test() {
        let src_file = NamedTempFile::new().unwrap();
        let dst_file = NamedTempFile::new().unwrap();
        let src = rusqlite::Connection::open(src_file.path().to_string_lossy().to_string()).unwrap();
        let dst = rusqlite::Connection::open(dst_file.path().to_string_lossy().to_string()).unwrap();

        src.execute("CREATE TABLE t(x INTEGER)", ()).unwrap();

        let backup = OnlineBackup::new(unsafe { src.handle() }, unsafe { dst.handle() }, true).unwrap();
        for step in backup {
            let step = step.unwrap();
            if let Some(locked_or_busy) = step.locked_or_busy {
                eprintln!("{locked_or_busy}");
            }
            eprintln!("{}/{}", step.pagecount - step.remaining, step.pagecount);
        }
    }
}
