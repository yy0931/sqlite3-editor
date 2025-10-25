use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::SingleStatement;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3PlaceholderKind;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedRange;
use serde::Deserialize;
use serde::Serialize;

/// Lists all placeholders in a statement.
///
/// For example, given `"SELECT ?1, $a, ?5, ?2, ?"` this function returns:
/// ```
/// [("?1", [7:9]), ("$a", [11:13, 19:21]), None, None, ("?5", [15:17]), (None, [23:24])]
/// //                       ^$a     ^2     ^^^^^^^^^                             ^?
/// //                                      implicitly inserted by ?5
/// ```
pub fn list_placeholders(stmt: &SingleStatement) -> Vec<Placeholder> {
    let mut result: Vec<Placeholder> = vec![];

    for token in &stmt.real_tokens {
        match &token.value {
            // https://www.sqlite.org/c3ref/bind_blob.html
            // ?
            SQLite3Token::Placeholder(_, SQLite3PlaceholderKind::Question) => {
                result.push(Placeholder::new(None, vec![token.range.clone()]));
            }
            // ?NNN
            SQLite3Token::Placeholder(s, SQLite3PlaceholderKind::QuestionNumber) => {
                if let Ok(n) = s[1..].parse::<usize>().map(|v| v - 1) {
                    while result.len() < n + 1 {
                        result.push(Placeholder::new(None, vec![]));
                    }
                    if result[n].name.is_none() {
                        result[n].name = Some(s.to_owned());
                    }
                    result[n].ranges_relative_to_stmt.push(token.range.clone());
                }
            }
            // :VVV, @VVV, $VVV
            SQLite3Token::Placeholder(
                s,
                SQLite3PlaceholderKind::AtName | SQLite3PlaceholderKind::ColonName | SQLite3PlaceholderKind::DollarName,
            ) => {
                result.push(Placeholder::new(Some(s.to_owned()), vec![token.range.clone()]));
            }
            _ => {}
        }
    }

    for placeholder in result.iter_mut() {
        for range in placeholder.ranges_relative_to_stmt.iter_mut() {
            range.start -= &stmt.real_text.range.start;
            range.end -= &stmt.real_text.range.start;
        }
    }
    result
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, derive_new::new, ts_rs::TS)]
#[ts(export)]
pub struct Placeholder {
    /// ? => None
    /// :VVV, @VVV, $VVV => Some(":VVV".to_owned()), ...
    pub name: Option<String>,

    /// Placeholders implicitly inserted by "?NNN" with skipped numbers (e.g., "SELECT ?1, ?4" -> ?2 and ?3 are implicitly inserted) => vec![]
    /// Others => ranges of the placeholders
    pub ranges_relative_to_stmt: Vec<ZeroIndexedRange>,
}

#[cfg(test)]
mod test {
    use super::list_placeholders;
    use crate::cli_subcommands::server::server_commands::code_lens::list_placeholders::Placeholder;
    use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::split_sqlite_statements;
    use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedRange;

    fn list(sql: &str) -> Vec<Placeholder> {
        let first_stmt = &split_sqlite_statements(sql).unwrap().0[0];
        list_placeholders(first_stmt)
    }

    #[test]
    pub fn test_list_placeholders_1() {
        assert_eq!(
            //    0000000000111
            //    0123456789012
            list("SELECT ?, $a"),
            vec![
                Placeholder::new(None, vec![ZeroIndexedRange::new4(0, 7, 0, 8)]), // ?
                Placeholder::new(Some("$a".to_owned()), vec![ZeroIndexedRange::new4(0, 10, 0, 12)]), // $a
            ]
        );
    }

    #[test]
    pub fn test_list_placeholders_2() {
        assert_eq!(
            //    0000000000111111111122222
            //    0123456789012345678901234
            list("SELECT ?1, $a, ?5, ?2, ?"),
            vec![
                Placeholder::new(Some("?1".to_owned()), vec![ZeroIndexedRange::new4(0, 7, 0, 9)]),
                Placeholder::new(
                    Some("$a".to_owned()),
                    vec![
                        ZeroIndexedRange::new4(0, 11, 0, 13),
                        ZeroIndexedRange::new4(0, 19, 0, 21)
                    ]
                ),
                Placeholder::new(None, vec![]),
                Placeholder::new(None, vec![]),
                Placeholder::new(Some("?5".to_owned()), vec![ZeroIndexedRange::new4(0, 15, 0, 17)]),
                Placeholder::new(None, vec![ZeroIndexedRange::new4(0, 23, 0, 24)]),
            ],
        );
    }
}

#[cfg(test)]
mod test_compare_sqlite {
    use crate::cli_subcommands::server::server_commands::code_lens::list_placeholders::list_placeholders;
    use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::split_sqlite_statements;

    fn list_placeholders_with_sqlite_api(sql: &str) -> rusqlite::Result<Vec<Option<String>>> {
        let conn = rusqlite::Connection::open_in_memory()?;
        let stmt = conn.prepare(sql)?;
        Ok((1..=stmt.parameter_count())
            .map(|i| stmt.parameter_name(i).map(|v| v.to_owned()))
            .collect())
    }

    fn compare(sql: &str) {
        let expected = list_placeholders_with_sqlite_api(sql).unwrap();
        let actual = list_placeholders(&split_sqlite_statements(sql).unwrap().0[0]);
        assert_eq!(expected.len(), actual.len());
        for i in 0..expected.len() {
            assert_eq!(actual[i].name, expected[i]);
        }
    }

    #[test]
    pub fn test_no_placeholder() {
        compare("SELECT 1");
    }

    #[test]
    pub fn test_placeholder_reuse() {
        compare("SELECT ?, ?, @a, ?2, ?3");
    }

    #[test]
    pub fn test_everything() {
        compare("WITH x AS (SELECT @a) SELECT ?, ?, ?10, :10, @10, $10, :aa, @aa, $aa, ?12, ?, :1a1");
    }

    #[test]
    pub fn test_issue_65() {
        compare("SELECT 1_2");
    }
}
