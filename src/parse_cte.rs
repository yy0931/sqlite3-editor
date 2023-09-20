use sqlparser::{
    keywords::Keyword,
    tokenizer::{Token, Word},
};

use crate::{split_statements::SplittedStatement, tokenize::ZeroIndexedLocation};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CTEEntry {
    pub ident_text: String,
    pub ident_start: ZeroIndexedLocation,
    pub ident_end: ZeroIndexedLocation,
    pub query_start: ZeroIndexedLocation,
    pub query_end: ZeroIndexedLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CTE {
    pub entries: Vec<CTEEntry>,
    pub body_start: ZeroIndexedLocation,
    pub body_end: ZeroIndexedLocation,
}

/// Parses the CTE in the given SQL statement.
/// Returns None if the statement does not have a CTE.
pub fn parse_cte(stmt: &SplittedStatement) -> Option<CTE> {
    // Return None if the first token is not "WITH"
    if !stmt.real_tokens.first().is_some_and(|t| match t.token {
        Token::Word(Word {
            keyword: Keyword::WITH, ..
        }) => true,
        _ => false,
    }) {
        return None;
    }

    let mut entries = Vec::<CTEEntry>::new();

    let mut paren_depth = 0;
    let mut current_entry: Option<CTEEntry> = None;

    for (i, token) in stmt.real_tokens.iter().enumerate() {
        // Update paren_depth
        match &token.token {
            Token::LParen => {
                paren_depth += 1;
            }
            Token::RParen => {
                paren_depth = (paren_depth - 1).max(0);
            }
            _ => {}
        }

        if let Some(current_entry_inner) = &mut current_entry {
            // Find the subquery
            match &token.token {
                Token::LParen if paren_depth == 1 => {
                    current_entry_inner.query_start = token.end.clone();
                    current_entry_inner.query_end = token.end.clone(); // placeholder
                    continue;
                }
                Token::RParen if paren_depth == 0 => {
                    current_entry_inner.query_end = token.start.clone();
                    entries.push(current_entry_inner.to_owned());
                    current_entry = None;
                }
                _ => {}
            }
        } else {
            // Find AS and the end of the WITH clause
            match &token.token {
                Token::Word(w) if paren_depth == 0 => match w.keyword {
                    Keyword::AS => {
                        // Find the last identifier
                        let mut paren_depth2 = 0;
                        for j in (0..(i - 1)).rev() {
                            match stmt.real_tokens[j].token {
                                Token::Whitespace(_) => {}
                                Token::LParen => { paren_depth2 -= 1;}
                                Token::RParen => { paren_depth2 += 1;}
                                _ if paren_depth2 == 0 => {
                                    current_entry = Some(CTEEntry {
                                        ident_text: match &stmt.real_tokens[j].token {
                                            Token::Word(word) => word.value.clone(),

                                            // this branch should not be used
                                            token => token.to_string(),
                                        },
                                        ident_start: stmt.real_tokens[j].start.clone(),
                                        ident_end: stmt.real_tokens[j].end.clone(),
                                        query_start: token.end.clone(), // placeholder
                                        query_end: token.end.clone(),   // placeholder
                                    });
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }

                    // > All common table expressions (ordinary and recursive) are created by prepending a WITH clause in front of a SELECT, INSERT, DELETE, or UPDATE statement.
                    // https://www.sqlite.org/lang_with.html
                    | Keyword::SELECT
                    | Keyword::INSERT
                    | Keyword::DELETE
                    | Keyword::UPDATE
                    | Keyword::REPLACE
                    | Keyword::MERGE
                    | Keyword::VALUES

                    // in case
                    | Keyword::CREATE
                    | Keyword::ALTER
                    | Keyword::DROP => {
                        return Some(CTE {
                            entries,
                            body_start: token.start.clone(),
                            body_end: stmt.real_end.clone(),
                        })
                    },
                    _ => {}
                },
                _ => {}
            }
        }
    }

    None
}
