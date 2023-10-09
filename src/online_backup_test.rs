use tempfile::NamedTempFile;

use crate::online_backup::OnlineBackup;

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
