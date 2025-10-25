use std::io::Write;
use std::path::PathBuf;

/// Copies a file.
///
/// This subcommand exists because Node.js’s file copy operation can produce
/// hard-to-debug error messages <https://github.com/yy0931/sqlite3-editor/issues/68#issuecomment-2675910142>.
/// This implementation provides clearer and more descriptive errors.
pub fn run(mut stderr: &mut impl Write, src: PathBuf, dst: PathBuf) -> i32 {
    // Read
    let data = match std::fs::read(&src) {
        Err(err) => {
            writeln!(&mut stderr, "Failed to read {src:?}: {err}").unwrap();
            return 1;
        }
        Ok(data) => data,
    };

    // Write
    if let Err(err) = std::fs::write(&dst, data) {
        writeln!(&mut stderr, "Failed to write to {dst:?}: {err}").unwrap();
        return 1;
    }

    0
}
