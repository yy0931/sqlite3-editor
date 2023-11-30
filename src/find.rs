use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use rusqlite::types::ValueRef;

lazy_static! {
    static ref REGEX_CACHE: Arc<Mutex<(String, regex::Regex)>> = Arc::new(Mutex::<(String, regex::Regex)>::new((
        "".to_owned(),
        regex::Regex::new("").unwrap(),
    )));
}

#[inline]
fn get_find_widget_input(ctx: &rusqlite::functions::Context) -> String {
    match ctx.get_raw(0) {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(v) => format!("{v}"),
        ValueRef::Real(v) => format!("{v}"),
        ValueRef::Text(v) => String::from_utf8_lossy(v).to_string(),
        ValueRef::Blob(_v) => "".to_owned(), // hex won't match against anything
    }
}

/// whole_word = true, case_sensitive = true, regex = false
pub fn find_widget_compare_w_c(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1).is_ok_and(|r| get_find_widget_input(ctx) == r) as i64
}

/// whole_word = true, case_sensitive = false, regex = false
pub fn find_widget_compare_w(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1)
        .is_ok_and(|r| get_find_widget_input(ctx).to_lowercase() == r.to_lowercase()) as i64
}

/// whole_word = false, case_sensitive = true, regex = false
pub fn find_widget_compare_c(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1)
        .is_ok_and(|r| get_find_widget_input(ctx).contains(&r)) as i64
}

/// whole_word = false, case_sensitive = false, regex = false
pub fn find_widget_compare(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1)
        .is_ok_and(|r| get_find_widget_input(ctx).to_lowercase().contains(&r.to_lowercase())) as i64
}

#[inline]
fn regex_match(text: &str, pattern: String) -> bool {
    {
        let regex_cached = REGEX_CACHE.lock().unwrap();
        if regex_cached.0 == pattern {
            return regex_cached.1.is_match(text);
        }
    }

    let Ok(v) = regex::Regex::new(&pattern) else {
        return false;
    };
    let matched = v.is_match(text);
    *REGEX_CACHE.lock().unwrap() = (pattern, v);
    matched
}

// whole_word = true, case_sensitive = true, regex = true
pub fn find_widget_compare_r_w_c(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1).is_ok_and(|pattern| {
        !pattern.is_empty() && regex_match(&get_find_widget_input(ctx), format!("(?s)\\b(?:{pattern})\\b"))
    }) as i64
}

/// whole_word = true, case_sensitive = false, regex = true
pub fn find_widget_compare_r_w(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1).is_ok_and(|pattern| {
        !pattern.is_empty() && regex_match(&get_find_widget_input(ctx), format!("(?i)(?s)\\b(?:{pattern})\\b"))
    }) as i64
}

/// whole_word = false, case_sensitive = true, regex = true
pub fn find_widget_compare_r_c(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1)
        .is_ok_and(|pattern| regex_match(&get_find_widget_input(ctx), format!("(?s){pattern}"))) as i64
}

/// whole_word = false, case_sensitive = false, regex = true
pub fn find_widget_compare_r(ctx: &rusqlite::functions::Context) -> i64 {
    ctx.get::<String>(1)
        .is_ok_and(|pattern| regex_match(&get_find_widget_input(ctx), format!("(?i)(?s){pattern}"))) as i64
}
