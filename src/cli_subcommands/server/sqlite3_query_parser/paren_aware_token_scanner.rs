use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::types::WithZeroIndexedRange;

/// Paren-aware token scanner that can walk tokens left-to-right (default Iterator) or right-to-left (via `.rev()`),
/// yielding tokens or parenthesized groups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SQLite3ParenAwareTokenScanner<'a> {
    tokens: &'a [WithZeroIndexedRange<SQLite3Token>],

    /// the next index from the left (inclusive)
    front: usize,

    /// the next index from the right (exclusive)
    back: usize,
    yielded_start: bool,
    yielded_end: bool,
}

impl<'a> SQLite3ParenAwareTokenScanner<'a> {
    pub fn new(tokens: &'a [WithZeroIndexedRange<SQLite3Token>]) -> Self {
        Self {
            tokens,
            front: 0,
            back: tokens.len(),
            yielded_start: false,
            yielded_end: false,
        }
    }

    /// Takes the next token from the left.
    /// Returns None when exhausted.
    fn pop_front(&mut self) -> Option<&'a WithZeroIndexedRange<SQLite3Token>> {
        if self.front >= self.back {
            return None;
        }
        let t = &self.tokens[self.front];
        self.front += 1;
        Some(t)
    }

    /// Takes the next token from the right.
    /// Returns None when exhausted.
    fn pop_back(&mut self) -> Option<&'a WithZeroIndexedRange<SQLite3Token>> {
        if self.front >= self.back {
            return None;
        }
        self.back -= 1;
        Some(&self.tokens[self.back])
    }
}

impl<'a> Iterator for SQLite3ParenAwareTokenScanner<'a> {
    type Item = SQLite3TokenScannerOutput<'a>;

    /// Takes the next token, grouping parentheses for left-to-right iteration.
    fn next(&mut self) -> Option<Self::Item> {
        // return StartOfInput once
        if !self.yielded_start && self.front == 0 {
            self.yielded_start = true;
            return Some(SQLite3TokenScannerOutput::StartOfInput);
        }

        // take the next token
        let Some(single_token) = self.pop_front() else {
            // return EndOfInput once
            if !self.yielded_end {
                self.yielded_end = true;
                return Some(SQLite3TokenScannerOutput::EndOfInput);
            }

            // return None if it's end of the token list
            return None;
        };

        // if it's not "(" then simply return it
        if single_token.value != SQLite3Token::LParen {
            return Some(SQLite3TokenScannerOutput::SingleToken(single_token));
        }

        // if it's "(" then find the matching ")" and return the tokens between "(" and ")"
        let index_after_paren = self.front;
        if let Some(inner) = scan_until_matching_paren(&mut || self.pop_front(), &SQLite3Token::LParen, &SQLite3Token::RParen) {
            return Some(SQLite3TokenScannerOutput::Group(inner));
        };

        // return "(" if the matching ")" was not found
        self.front = index_after_paren;
        Some(SQLite3TokenScannerOutput::SingleToken(single_token))
    }
}

impl<'a> DoubleEndedIterator for SQLite3ParenAwareTokenScanner<'a> {
    /// Takes the next token from the right, grouping parentheses for right-to-left iteration.
    fn next_back(&mut self) -> Option<Self::Item> {
        // return EndOfInput once
        if !self.yielded_end && self.back == self.tokens.len() {
            self.yielded_end = true;
            return Some(SQLite3TokenScannerOutput::EndOfInput);
        }

        // take the next token
        let Some(single_token) = self.pop_back() else {
            // return StartOfInput once
            if !self.yielded_start {
                self.yielded_start = true;
                return Some(SQLite3TokenScannerOutput::StartOfInput);
            }

            // return None if it's end of the token list
            return None;
        };

        // if it's not ")" then simply return it
        if single_token.value != SQLite3Token::RParen {
            return Some(SQLite3TokenScannerOutput::SingleToken(single_token));
        }

        // if it's ")" then find the matching "(" and return the tokens between ")" and "("
        let index_before_paren = self.back;
        if let Some(mut inner) = scan_until_matching_paren(&mut || self.pop_back(), &SQLite3Token::RParen, &SQLite3Token::LParen) {
            inner.reverse();
            return Some(SQLite3TokenScannerOutput::Group(inner));
        };

        // return ")" if the matching "(" was not found
        self.back = index_before_paren;
        Some(SQLite3TokenScannerOutput::SingleToken(single_token))
    }
}

impl<'a> std::iter::FusedIterator for SQLite3ParenAwareTokenScanner<'a> {}

/// Consumes tokens until the matching parenthesis is found, returning the inner tokens (or `None` if unbalanced).
fn scan_until_matching_paren<'a>(
    take_token: &mut impl FnMut() -> Option<&'a WithZeroIndexedRange<SQLite3Token>>,
    opening_paren: &SQLite3Token,
    closing_paren: &SQLite3Token,
) -> Option<Vec<&'a WithZeroIndexedRange<SQLite3Token>>> {
    let mut depth = 1;
    let mut inner = vec![];

    while let Some(t) = take_token() {
        if t.value == *opening_paren {
            depth += 1
        } else if t.value == *closing_paren {
            depth -= 1;
            if depth == 0 {
                return Some(inner);
            }
        }
        inner.push(t);
    }
    None
}

/// Output produced by the paren-aware token scanners: either a single token or a parenthesized group.
///
/// "1 + (2 + 3)" -> [SingleToken("1"), SingleToken(" "), SingleToken("+"), SingleToken(" "), Group("2 + 3")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SQLite3TokenScannerOutput<'a> {
    /// a top-level token
    SingleToken(&'a WithZeroIndexedRange<SQLite3Token>),

    /// a parenthesized group
    /// `.0` contains the tokens between "(" and ")".
    /// The order of tokens is still left-to-right when iterating from right to left.
    Group(Vec<&'a WithZeroIndexedRange<SQLite3Token>>),

    /// This token is yielded only once at the start of input (emitted last when iterating from right to left).
    StartOfInput,
    /// This token is yielded only once at the end of input (emitted first when iterating from right to left).
    EndOfInput,
}

#[cfg(test)]
mod test {
    use std::borrow::Borrow;

    use super::SQLite3ParenAwareTokenScanner;
    use super::SQLite3TokenScannerOutput;
    use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
    use crate::cli_subcommands::server::sqlite3_query_parser::tokenize::tokenize_with_range_location;

    fn consume_start_of_input<'a>(it: &mut impl Iterator<Item = SQLite3TokenScannerOutput<'a>>) {
        assert_eq!(it.next().unwrap(), SQLite3TokenScannerOutput::StartOfInput);
    }

    fn consume_end_of_input<'a>(it: &mut impl Iterator<Item = SQLite3TokenScannerOutput<'a>>) {
        assert_eq!(it.next().unwrap(), SQLite3TokenScannerOutput::EndOfInput);
    }

    fn unwrap_single_token<'a>(output: impl Borrow<SQLite3TokenScannerOutput<'a>>) -> SQLite3Token {
        match output.borrow() {
            SQLite3TokenScannerOutput::SingleToken(token) => token.value.clone(),
            _ => panic!(),
        }
    }

    fn unwrap_group<'a>(output: impl Borrow<SQLite3TokenScannerOutput<'a>>) -> Vec<SQLite3Token> {
        match output.borrow() {
            SQLite3TokenScannerOutput::Group(tokens) => tokens.iter().map(|t| t.value.clone()).collect::<Vec<_>>(),
            _ => panic!(),
        }
    }

    #[test]
    fn ltr_empty_input_returns_none() {
        let tokens = tokenize_with_range_location("").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens);
        consume_start_of_input(&mut it);
        consume_end_of_input(&mut it);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn rtl_empty_input_returns_none() {
        let tokens = tokenize_with_range_location("").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens).rev();
        consume_end_of_input(&mut it);
        consume_start_of_input(&mut it);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn ltr_unmatched_right_paren_yields_single_right_paren() {
        let tokens = tokenize_with_range_location(") 1").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens);
        consume_start_of_input(&mut it);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::RParen);
    }

    #[test]
    fn ltr_unmatched_left_paren_yields_single_left_paren() {
        let tokens = tokenize_with_range_location("(1 + 2").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens);
        consume_start_of_input(&mut it);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::LParen);
    }

    #[test]
    fn rtl_unmatched_left_paren_yields_single_left_paren() {
        let tokens = tokenize_with_range_location("(").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens).rev();
        consume_end_of_input(&mut it);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::LParen);
    }

    #[test]
    fn rtl_unmatched_right_paren_yields_single_right_paren() {
        let tokens = tokenize_with_range_location(")").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens).rev();
        consume_end_of_input(&mut it);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::RParen);
    }

    #[test]
    fn ltr_simple_group_empty_between_parens() {
        let tokens = tokenize_with_range_location("()").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens);
        consume_start_of_input(&mut it);
        assert_eq!(unwrap_group(it.next().unwrap()), []);
    }

    #[test]
    fn rtl_simple_group_empty_between_parens() {
        let tokens = tokenize_with_range_location("()").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens).rev();
        consume_end_of_input(&mut it);
        assert_eq!(unwrap_group(it.next().unwrap()), []);
    }

    #[test]
    fn ltr_nested_parens_group_contains_inner_pair_in_order() {
        let tokens = tokenize_with_range_location("(())").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens);
        consume_start_of_input(&mut it);
        assert_eq!(unwrap_group(it.next().unwrap()), [SQLite3Token::LParen, SQLite3Token::RParen]);
    }

    #[test]
    fn rtl_nested_parens_group_contains_inner_pair_in_order() {
        let tokens = tokenize_with_range_location("(())").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens).rev();
        consume_end_of_input(&mut it);
        assert_eq!(unwrap_group(it.next().unwrap()), [SQLite3Token::LParen, SQLite3Token::RParen]);
    }

    #[test]
    fn ltr_consumes_multiple_items_until_exhaustion() {
        let tokens = tokenize_with_range_location("1 () () 2").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens);
        consume_start_of_input(&mut it);
        assert_eq!(
            unwrap_single_token(it.next().unwrap()),
            SQLite3Token::NumericLiteral("1".to_owned())
        );
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::Whitespace(false));
        assert_eq!(unwrap_group(it.next().unwrap()), []);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::Whitespace(false));
        assert_eq!(unwrap_group(it.next().unwrap()), []);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::Whitespace(false));
        assert_eq!(
            unwrap_single_token(it.next().unwrap()),
            SQLite3Token::NumericLiteral("2".to_owned())
        );
        consume_end_of_input(&mut it);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn rtl_consumes_multiple_items_until_exhaustion() {
        let tokens = tokenize_with_range_location("1 () () 2").unwrap();
        let mut it = SQLite3ParenAwareTokenScanner::new(&tokens).rev();
        consume_end_of_input(&mut it);
        assert_eq!(
            unwrap_single_token(it.next().unwrap()),
            SQLite3Token::NumericLiteral("2".to_owned())
        );
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::Whitespace(false));
        assert_eq!(unwrap_group(it.next().unwrap()), []);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::Whitespace(false));
        assert_eq!(unwrap_group(it.next().unwrap()), []);
        assert_eq!(unwrap_single_token(it.next().unwrap()), SQLite3Token::Whitespace(false));
        assert_eq!(
            unwrap_single_token(it.next().unwrap()),
            SQLite3Token::NumericLiteral("1".to_owned())
        );
        consume_start_of_input(&mut it);
        assert_eq!(it.next(), None);
    }
}
