use serde::Deserialize;
use serde::Serialize;

/// Represents a location within text, using zero-indexing for both line and column.
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize, derive_new::new, ts_rs::TS)]
#[ts(export)]
pub struct ZeroIndexedLocation {
    #[ts(type = "bigint")]
    pub line: usize,
    #[ts(type = "bigint")]
    pub column: usize,
}

impl std::ops::Sub<&Self> for ZeroIndexedLocation {
    type Output = Self;
    fn sub(self, rhs: &Self) -> Self::Output {
        Self {
            line: self.line.saturating_sub(rhs.line),
            column: if self.line == rhs.line {
                self.column.saturating_sub(rhs.column)
            } else {
                self.column
            },
        }
    }
}

impl std::ops::SubAssign<&Self> for ZeroIndexedLocation {
    fn sub_assign(&mut self, rhs: &Self) {
        if self.line == rhs.line {
            self.column = self.column.saturating_sub(rhs.column);
        }
        // Update line after comparing it to rhs.line
        self.line = self.line.saturating_sub(rhs.line);
    }
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
        write!(f, "[{}:{}]", self.line, self.column)
    }
}

impl From<sqlparser::tokenizer::Location> for ZeroIndexedLocation {
    fn from(value: sqlparser::tokenizer::Location) -> Self {
        Self {
            line: value.line.checked_sub(1).unwrap().try_into().unwrap(),
            column: value.column.checked_sub(1).unwrap().try_into().unwrap(),
        }
    }
}
