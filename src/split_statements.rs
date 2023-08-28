use std::borrow::Cow;

use sqlparser::{
    dialect::SQLiteDialect,
    keywords::Keyword,
    tokenizer::{Token, TokenizerError, Word},
};

use crate::tokenize::{tokenize_with_range_location, TokenWithRangeLocation, ZeroIndexedLocation};

/// Represents a split SQL statement with its text, actual text, and their locations.
/// ```plaintext
/// real_start  real_end
///    v        v
/// "  SELECT 1;  "
///  ^            ^
/// start        end
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct SplittedStatement {
    pub text: String,
    pub real_text: String,
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
    pub real_start: ZeroIndexedLocation,
    pub real_end: ZeroIndexedLocation,
    pub real_tokens: Vec<TokenWithRangeLocation>,
}

impl std::fmt::Debug for SplittedStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{
    raw:  {:?}-{:?}: {:?}
    real: {:?}-{:?}: {:?}
}}",
            self.start, self.end, self.text, self.real_start, self.real_end, self.real_text
        )
    }
}

impl SplittedStatement {
    fn new(
        lines: &[&str],
        tokens: &[&TokenWithRangeLocation],
        start: ZeroIndexedLocation,
        end: ZeroIndexedLocation,
    ) -> Self {
        let real_start_i = tokens.iter().position(|token| match token.token {
            Token::Whitespace(_) => false,
            _ => true,
        });
        let real_end_i = tokens
            .iter()
            .rev()
            .position(|token| match token.token {
                Token::Whitespace(_) => false,
                _ => true,
            })
            .map(|i| tokens.len() - 1 - i);

        if let (Some(real_start_i), Some(real_end_i)) = (real_start_i, real_end_i) {
            let real_start = tokens[real_start_i].start.to_owned();
            let real_end = tokens[real_end_i].end.to_owned();
            Self {
                text: get_text_range(&lines, &start, &end),
                real_text: get_text_range(&lines, &real_start, &real_end),
                start,
                end,
                real_start: real_start.to_owned(),
                real_end: real_end.to_owned(),
                real_tokens: tokens[real_start_i..(real_end_i + 1)]
                    .into_iter()
                    .map(|&t| t.to_owned())
                    .collect::<Vec<_>>(),
            }
        } else {
            Self {
                text: get_text_range(&lines, &start, &end),
                real_text: get_text_range(&lines, &start, &end),
                start: start.clone(),
                end: end.clone(),
                real_start: start,
                real_end: end,
                real_tokens: tokens.into_iter().map(|&t| t.to_owned()).collect::<Vec<_>>(),
            }
        }
    }
}

/// Splits the input SQL string into a vector of `SplittedStatement`.
pub fn split_sqlite_statements(sql: &str) -> Result<Vec<SplittedStatement>, TokenizerError> {
    let lines = sql.lines().collect::<Vec<_>>();
    let mut stmt_start = ZeroIndexedLocation { column: 0, line: 0 };
    let mut stmt_tokens = vec![];
    let mut result: Vec<SplittedStatement> = vec![];

    // BEGIN ... (END or COMMIT or ROLLBACK) https://www.sqlite.org/lang_transaction.html
    let mut begin_end_block_depth = 0;

    let tokens = tokenize_with_range_location(&SQLiteDialect {}, sql)?;
    for token_with_location in &tokens {
        stmt_tokens.push(token_with_location);
        let TokenWithRangeLocation { token, end, .. } = token_with_location;
        match token {
            Token::Word(Word {
                keyword: Keyword::BEGIN | Keyword::CASE,
                ..
            }) => {
                begin_end_block_depth += 1;
            }
            Token::Word(Word { keyword, .. })
                if match keyword {
                    Keyword::END | Keyword::COMMIT | Keyword::ROLLBACK => true,
                    _ => false,
                } =>
            {
                begin_end_block_depth -= 1;
                if begin_end_block_depth < 0 {
                    begin_end_block_depth = 0;
                }
            }
            Token::SemiColon if begin_end_block_depth == 0 => {
                result.push(SplittedStatement::new(&lines, &stmt_tokens, stmt_start, end.to_owned()));
                stmt_start = end.to_owned();
                stmt_tokens.clear();
            }
            _ => {}
        }
    }

    if let Some(last) = tokens.last() {
        if stmt_start != last.end {
            result.push(SplittedStatement::new(
                &lines,
                &stmt_tokens,
                stmt_start,
                last.end.clone(),
            ));
        }
    }

    Ok(result)
}

/// Extracts the text between the provided start and end locations from the given lines.
///
/// # Arguments
/// * `lines` - A slice of string slices representing lines of text.
/// * `start` - The start location represented as ZeroIndexedLocation.
/// * `end` - The end location represented as ZeroIndexedLocation.
pub fn get_text_range<'a>(lines: &[&'a str], start: &ZeroIndexedLocation, end: &ZeroIndexedLocation) -> String {
    if start.line == end.line {
        // If start and end are on the same line
        slice_unicode_str(&lines[start.line], Some(start.column), Some(end.column))
    } else {
        // If start and end are on different lines
        let mut result: Vec<Cow<str>> = vec![];
        // Add the rest of the first line
        result.push(Cow::Owned(slice_unicode_str(
            &lines[start.line],
            Some(start.column),
            None,
        )));
        // Add the complete lines between start and end
        for line in &lines[start.line + 1..end.line] {
            result.push(Cow::Borrowed(line));
        }
        // Add the part of the last line
        result.push(Cow::Owned(slice_unicode_str(&lines[end.line], None, Some(end.column))));
        result.join("\n")
    }
}

/// Returns a Unicode-aware substring of the given string `s` using the specified start and end indices.
/// If start or end is `None`, it defaults to the start or end of the string respectively.
///
/// # Arguments
/// * `s` - A string slice that you want to get a substring of.
/// * `start` - Optional index for where the substring starts.
/// * `end` - Optional index for where the substring ends.
fn slice_unicode_str(s: &str, start: Option<usize>, end: Option<usize>) -> String {
    let start = start.unwrap_or(0);
    let end = end.unwrap_or_else(|| s.chars().count());
    assert!(start <= end, "Invalid range: start index is greater than end index.");
    s.chars().skip(start).take(end - start).collect()
}
