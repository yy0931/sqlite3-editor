/// Escapes an identifier for SQLite3.
pub fn escape_sql_identifier(ident: &str) -> String {
    if ident.contains('\x00') {
        panic!("Failed to quote the SQL identifier {ident:?} as it contains a NULL char");
    }
    format!("`{}`", ident.replace('`', "``"))
}
