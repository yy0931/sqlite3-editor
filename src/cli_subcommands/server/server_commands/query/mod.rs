pub mod request_type;
pub mod sqlite3;

use std::fs::File;
use std::io::Write;

use crate::cli_error::CLIErrorCode;
use crate::cli_subcommands::server::server_commands::query::request_type::QueryCommandParams;
use crate::cli_subcommands::server::server_commands::query::sqlite3::connection::SQLite3Connection;
use crate::cli_subcommands::server::TruncateAll;
use crate::msgpack::read_msgpack_into_json;

pub fn run(db: &mut SQLite3Connection, r: &mut File, w: &mut File) -> CLIErrorCode {
    // Deserialize the request
    let req = match rmp_serde::from_read::<_, QueryCommandParams>(&mut *r) {
        Ok(req) => req,
        Err(err) => {
            let mut content = read_msgpack_into_json(r);
            if content.len() > 5000 {
                content = content[0..5000].to_owned() + "... (omitted)"
            }
            write!(
                w,
                "Failed to parse the request body: {err} (content = {content}, len = {})",
                r.metadata().unwrap().len()
            )
            .expect("Failed to write an error message.");
            return CLIErrorCode::OtherError;
        }
    };

    match db.handle(w, &req.query, &req.params, req.mode, req.options) {
        Ok(_) => CLIErrorCode::Success,
        Err(err) => {
            w.truncate_all();
            write!(w, "{err}").expect("Failed to write an error message.");
            err.code()
        }
    }
}
