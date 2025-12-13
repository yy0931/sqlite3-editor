use crate::cli_error::CLIError;
use base64::engine::general_purpose;
use base64::Engine as _;
use rusqlite::types::ValueRef;
use std::io::Write;

/// Exports a table as JSON.
/// - BLOB values are encoded as BASE64 strings.
pub fn export<W: Write>(database_filepath: &str, query: &str, mut writer: &mut W) -> std::result::Result<(), CLIError> {
    // Query
    let con = super::connect(database_filepath)?;
    let mut stmt = con.prepare(query).or_else(|err| CLIError::new_query_error(err, query, &[]))?;

    // TODO: `stmt.column_names()` should be called after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53).
    let column_names = stmt.column_names().into_iter().map(|v| v.to_owned()).collect::<Vec<_>>();

    writer.write_all(b"[")?;
    let mut rows = stmt.query([]).or_else(|err| CLIError::new_query_error(err, query, &[]))?;
    let mut first_entry = true;
    while let Some(row) = rows.next().or_else(|err| CLIError::new_query_error(err, query, &[]))? {
        if !first_entry {
            writer.write_all(b",")?;
        }
        first_entry = false;
        writer.write_all(b"{")?;
        for (col_id, column_name) in column_names.iter().enumerate() {
            if col_id != 0 {
                writer.write_all(b",")?;
            }
            serde_json::to_writer::<&mut W, _>(&mut writer, &column_name)?;
            writer.write_all(b":")?;
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => {
                    writer.write_all(b"null")?;
                }
                ValueRef::Real(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &v)?;
                }
                ValueRef::Blob(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &general_purpose::STANDARD.encode(v))?;
                }
                ValueRef::Integer(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &v)?;
                }
                ValueRef::Text(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &String::from_utf8_lossy(v))?;
                }
            }
        }
        writer.write_all(b"}")?;
    }
    writer.write_all(b"]")?;

    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_export_json() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();
        let mut buf = Vec::new();

        super::super::setup_test_db(&tmp_db_filepath);

        super::export(&tmp_db_filepath, "SELECT * FROM test", &mut buf).unwrap();

        assert_eq!(
            String::from_utf8(buf).unwrap(),
            r#"[{"t":"Alice","i":20,"n":null,"r":1.2,"b":"AQID"},{"t":"Alice","i":25,"n":null,"r":2.4,"b":"BAUG"}]"#
        );
    }
}
