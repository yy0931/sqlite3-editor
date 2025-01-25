use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlparser::{
    keywords::Keyword,
    tokenizer::{Token, Word},
};

use crate::{
    split_statements::SplittedStatement,
    tokenize::{TokenWithRangeLocation, ZeroIndexedLocation},
};

static QUESTION_NUMBER: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"^\?\d+$").unwrap());

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[ts(export)]
pub struct PlaceholderRange {
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
}

impl PlaceholderRange {
    fn new(token: &TokenWithRangeLocation) -> Self {
        Self {
            start: token.start.clone(),
            end: token.end.clone(),
        }
    }
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[ts(export)]
pub struct Placeholder {
    /// None for "?", Some for "?NNN", ":VVV", etc.
    pub name: Option<String>,
    /// Empty for placeholders implicitly inserted by "?NNN".
    pub ranges_relative_to_stmt: Vec<PlaceholderRange>,
}

pub fn list_placeholders(stmt: &SplittedStatement) -> Vec<Placeholder> {
    let mut previous_colon_or_at_sign: Option<String> = None;
    let mut result: Vec<Placeholder> = vec![];
    let mut is_previous_placeholder_unfinished = false;

    for token in &stmt.real_tokens {
        if is_previous_placeholder_unfinished {
            match &token.token {
                Token::Word(Word {
                    value: s,
                    quote_style: None,
                    keyword: Keyword::NoKeyword,
                })
                | Token::Number(s, /* "L" suffix */ false) => {
                    // Extend the last placeholder
                    result.last_mut().unwrap().name =
                        Some(result.last().unwrap().name.to_owned().unwrap() + s.as_str());
                    result
                        .last_mut()
                        .unwrap()
                        .ranges_relative_to_stmt
                        .last_mut()
                        .unwrap()
                        .end = token.end.clone();
                }
                _ => {
                    is_previous_placeholder_unfinished = false;
                }
            }
        }
        if !is_previous_placeholder_unfinished {
            match &token.token {
                // https://www.sqlite.org/c3ref/bind_blob.html
                // ?
                Token::Placeholder(p) if p == "?" => {
                    result.push(Placeholder {
                        name: None,
                        ranges_relative_to_stmt: vec![PlaceholderRange::new(token)],
                    });
                }
                // ?NNN
                Token::Placeholder(s) if QUESTION_NUMBER.is_match(s) => {
                    if let Ok(n) = s[1..].parse::<usize>().map(|v| v - 1) {
                        while result.len() < n + 1 {
                            result.push(Placeholder {
                                name: None,
                                ranges_relative_to_stmt: vec![],
                            });
                        }
                        if result[n].name.is_none() {
                            result[n].name = Some(s.to_owned());
                            result[n].ranges_relative_to_stmt.push(PlaceholderRange::new(token));
                        }
                    }
                }
                // :VVV, @VVV, $VVV
                Token::Word(Word {
                    value: s,
                    quote_style: None,
                    keyword: Keyword::NoKeyword,
                }) => {
                    if s.starts_with(":") || s.starts_with("@") || s.starts_with("$") {
                        result.push(Placeholder {
                            name: Some(s.to_owned()),
                            ranges_relative_to_stmt: vec![PlaceholderRange::new(token)],
                        });
                        is_previous_placeholder_unfinished = true;
                    } else if let Some(sign) = previous_colon_or_at_sign {
                        let mut range = PlaceholderRange::new(token);
                        range.start.column = range.start.column.saturating_sub(1);
                        result.push(Placeholder {
                            name: Some(sign + s.as_str()),
                            ranges_relative_to_stmt: vec![range],
                        });
                        is_previous_placeholder_unfinished = true;
                    }
                }
                Token::Number(s, /* "L" suffix */ false) => {
                    if let Some(sign) = previous_colon_or_at_sign {
                        let mut range = PlaceholderRange::new(token);
                        range.start.column = range.start.column.saturating_sub(1);
                        result.push(Placeholder {
                            name: Some(sign + s.as_str()),
                            ranges_relative_to_stmt: vec![range],
                        });
                        is_previous_placeholder_unfinished = true;
                    }
                }
                _ => {}
            }
        }
        previous_colon_or_at_sign = match token.token {
            Token::Colon => Some(":".to_owned()),
            Token::AtSign => Some("@".to_owned()),
            _ => None,
        };
    }

    for placeholder in result.iter_mut() {
        for range in placeholder.ranges_relative_to_stmt.iter_mut() {
            range.start -= &stmt.real_start;
            range.end -= &stmt.real_start;
        }
    }
    result
}
