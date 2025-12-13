use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

use crate::cli_subcommands::server::sqlite3_query_parser::types::location::ZeroIndexedLocation;

/// Represents a range within text, using zero-indexing for both line and column.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, derive_new::new, ts_rs::TS)]
#[ts(export)]
pub struct ZeroIndexedRange {
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
}

impl ZeroIndexedRange {
    pub fn new4(start_line: usize, start_column: usize, end_line: usize, end_column: usize) -> Self {
        Self::new(
            ZeroIndexedLocation::new(start_line, start_column),
            ZeroIndexedLocation::new(end_line, end_column),
        )
    }

    /// Extracts the text covered by the range.
    pub fn get_text(&self, lines: &[&str]) -> String {
        if self.start.line == self.end.line {
            // If start and end are on the same line
            slice_unicode_str(lines[self.start.line], Some(self.start.column), Some(self.end.column))
        } else {
            // If start and end are on different lines
            let mut result: Vec<Cow<str>> = vec![];
            // Add the rest of the first line
            result.push(Cow::Owned(slice_unicode_str(lines[self.start.line], Some(self.start.column), None)));
            // Add the complete lines between start and end
            for line in &lines[self.start.line + 1..self.end.line] {
                result.push(Cow::Borrowed(line));
            }
            // Add the part of the last line
            result.push(Cow::Owned(slice_unicode_str(lines[self.end.line], None, Some(self.end.column))));
            result.join("\n")
        }
    }

    /// Extracts the text covered by the range, with the range information preserved.
    pub fn get_text_with_range(&self, lines: &[&str]) -> WithZeroIndexedRange<String> {
        WithZeroIndexedRange {
            value: self.get_text(lines),
            range: self.clone(),
        }
    }
}

/// Represents a value along with its range location.
#[derive(Clone, Eq, PartialEq)]
pub struct WithZeroIndexedRange<T> {
    pub value: T,
    pub range: ZeroIndexedRange,
}

impl<T: std::fmt::Debug> std::fmt::Debug for WithZeroIndexedRange<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{{}:{}-{}:{} {:?}}}",
            self.range.start.line, self.range.start.column, self.range.end.line, self.range.end.column, self.value
        )
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
    s.chars()
        .skip(start)
        .take(
            end.checked_sub(start)
                .expect("Invalid range: start index is greater than end index."),
        )
        .collect()
}
