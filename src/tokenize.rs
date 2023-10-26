use serde::{Deserialize, Serialize};
use sqlparser::dialect::Dialect;
use sqlparser::tokenizer::{Location, Token, TokenWithLocation, Tokenizer, TokenizerError, Whitespace};

/// A struct representing a location within text, using zero-indexing for both line and column.
#[derive(ts_rs::TS, Eq, PartialEq, Clone, Serialize, Deserialize)]
#[ts(export)]
pub struct ZeroIndexedLocation {
    #[ts(type = "bigint")]
    pub line: usize,
    #[ts(type = "bigint")]
    pub column: usize,
}

impl std::cmp::Ord for ZeroIndexedLocation {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then_with(|| self.column.cmp(&other.column))
    }
}

impl std::cmp::PartialOrd for ZeroIndexedLocation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ZeroIndexedLocation {
    /// Calculates the character offset of a given location in a string.
    ///
    /// # Arguments
    ///
    /// * `s` - The input string.
    /// * `location` - The location within the string for which to calculate the character offset.
    ///
    /// # Returns
    ///
    /// The character offset as usize.
    pub fn offset_at(&self, s: &str) -> usize {
        let ZeroIndexedLocation { line, column } = self;
        let mut offset = 0;
        for (i, line_str) in s.split('\n').enumerate() {
            if i >= *line {
                break;
            }
            offset += line_str.chars().count() + 1;
        }
        offset + column
    }
}

impl std::fmt::Debug for ZeroIndexedLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<line {}, column {}>", self.line, self.column)
    }
}

impl ZeroIndexedLocation {
    // Creates a new ZeroIndexedLocation with the given line and column.
    #[allow(dead_code)]
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl From<Location> for ZeroIndexedLocation {
    fn from(value: Location) -> Self {
        Self {
            line: value.line as usize - 1,
            column: value.column as usize - 1,
        }
    }
}

/// Represents a token along with its range location in the input string.
#[derive(Clone, PartialEq, Eq)]
pub struct TokenWithRangeLocation {
    pub token: Token,
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
}

impl std::fmt::Debug for TokenWithRangeLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{:?}-{:?}: {:?}}}", self.start, self.end, self.token)
    }
}

/// Tokenizes the given SQL input string and appends the end location to each token.
///
/// # Arguments
///
/// * `dialect` - The SQL dialect to be used for tokenization.
/// * `sql` - The input SQL string.
pub fn tokenize_with_range_location(
    dialect: &dyn Dialect,
    sql: &str,
) -> Result<Vec<TokenWithRangeLocation>, TokenizerError> {
    // Tokenize the SQL query
    let tokens = Tokenizer::new(dialect, sql).tokenize_with_location()?;

    // Merge whitespace tokens, e.g. [A, " ", " ", B] -> [A, "  ", B]
    let mut tokens_merged: Vec<TokenWithLocation> = vec![];
    for token in tokens {
        match token.token {
            Token::Whitespace(Whitespace::Space) | Token::Whitespace(Whitespace::Tab)
                if tokens_merged.last().map(|t| &t.token) == Some(&token.token) =>
            {
                continue
            }
            _ => {
                tokens_merged.push(token);
            }
        }
    }
    let tokens = tokens_merged;

    // Add range location to each token
    Ok(tokens
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, token)| {
            let start = token.location.into();
            let end = match tokens.get(i + 1) {
                Some(next_token) => next_token.location.clone().into(),
                None => {
                    // The location of the end of file
                    let lines = sql.lines().collect::<Vec<_>>();
                    ZeroIndexedLocation {
                        line: lines.len().max(1) - 1,
                        column: lines.last().map_or(0, |line| line.len()),
                    }
                }
            };
            TokenWithRangeLocation {
                token: token.token,
                start,
                end,
            }
        })
        .collect::<Vec<_>>())
}
