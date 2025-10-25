use crate::cli_error::CLIError;
use base64::engine::general_purpose;
use base64::Engine as _;
use rusqlite::types::ValueRef;
use serde::Deserialize;
use serde::Serialize;
use std::io::Write;

/// Exports a table as CSV.
/// - NULL is encoded as an empty string.
/// - BLOB values are encoded as BASE64 strings.
pub fn export<W: Write>(
    database_filepath: &str,
    query: &str,
    writer: &mut W,
    options: &CSVExportOptions,
) -> std::result::Result<(), CLIError> {
    // Query
    let con = super::connect(database_filepath)?;
    let mut stmt = con
        .prepare(query)
        .or_else(|err| CLIError::new_query_error(err, query, &[]))?;

    if options.delimiter.len() != 1 {
        CLIError::new_other_error("The delimiter needs to be a single-byte character.", None, None)?;
    }

    // TODO: `stmt.column_count()` and `stmt.column_names()` should be called after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53).
    let column_count = stmt.column_count();
    let column_names = stmt
        .column_names()
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();

    let mut w = csv::WriterBuilder::new()
        .delimiter(options.delimiter.as_bytes()[0])
        .from_writer(writer);

    // Header
    for name in column_names {
        w.write_field(name)?;
    }
    w.write_record(None::<&[u8]>)?;

    let mut rows = stmt
        .query([])
        .or_else(|err| CLIError::new_query_error(err, query, &[]))?;
    while let Some(row) = rows.next().or_else(|err| CLIError::new_query_error(err, query, &[]))? {
        for col_id in 0..column_count {
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => w.write_field(&options.null)?,
                ValueRef::Real(v) => w.write_field(format!("{v}"))?,
                ValueRef::Blob(v) => w.write_field(general_purpose::STANDARD.encode(v))?,
                ValueRef::Integer(v) => w.write_field(format!("{v}"))?,
                ValueRef::Text(v) => w.write_field(v)?,
            };
        }
        w.write_record(None::<&[u8]>)?;
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct CSVExportOptions {
    pub delimiter: String,
    pub null: String,
}

impl Default for CSVExportOptions {
    fn default() -> Self {
        Self {
            delimiter: ",".to_owned(),
            null: "".to_owned(),
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_export_csv() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();
        let mut buf = Vec::new();

        super::super::setup_test_db(&tmp_db_filepath);

        super::export(
            &tmp_db_filepath,
            "SELECT * FROM test",
            &mut buf,
            &super::CSVExportOptions::default(),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "t,i,n,r,b\nAlice,20,,1.2,AQID\nAlice,25,,2.4,BAUG\n"
        );
    }

    #[test]
    fn test_export_tsv() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();
        let mut buf = Vec::new();

        super::super::setup_test_db(&tmp_db_filepath);

        super::export(
            &tmp_db_filepath,
            "SELECT * FROM test",
            &mut buf,
            &super::CSVExportOptions {
                delimiter: "\t".to_owned(),
                null: "".to_owned(),
            },
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "t\ti\tn\tr\tb\nAlice\t20\t\t1.2\tAQID\nAlice\t25\t\t2.4\tBAUG\n"
        );
    }

    #[test]
    fn test_invalid_delimiter() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();
        let mut buf = Vec::new();

        super::super::setup_test_db(&tmp_db_filepath);

        assert!(super::export(
            &tmp_db_filepath,
            "SELECT * FROM test",
            &mut buf,
            &super::CSVExportOptions {
                delimiter: ",,".to_owned(),
                null: "".to_owned(),
            }
        )
        .unwrap_err()
        .to_string()
        .contains("The delimiter needs to be a single-byte character."));
    }
}
