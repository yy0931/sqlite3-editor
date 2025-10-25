use crate::cli_subcommands::server::sqlite3_fns::column_origin::ColumnOrigin;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableSchemaColumnForeignKey {
    #[ts(type = "bigint")]
    pub id: i64,
    #[ts(type = "bigint")]
    pub seq: i64,
    pub table: String,
    pub to: String,
    pub on_update: String,
    pub on_delete: String,
    #[serde(rename = "match")]
    pub match_: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableSchemaColumn {
    #[ts(type = "bigint")]
    pub cid: i64,
    pub dflt_value: Option<String>,
    pub name: String,
    pub notnull: bool,
    #[serde(rename = "type")]
    pub type_: String,
    pub pk: bool,
    #[serde(rename = "autoIncrement")]
    pub auto_increment: bool,
    #[serde(rename = "foreignKeys")]
    pub foreign_keys: Vec<TableSchemaColumnForeignKey>,
    /** 1: columns in virtual tables, 2: dynamic generated columns, 3: stored generated columns */
    #[ts(type = "0n | 1n | 2n | 3n")]
    pub hidden: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct IndexColumn {
    #[ts(type = "bigint")]
    pub seqno: i64,
    #[ts(type = "bigint")]
    pub cid: i64,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableSchemaIndex {
    pub seq: i64,
    pub name: String,
    #[ts(type = "0n | 1n")]
    pub unique: i64,
    #[ts(type = "'c' | 'u' | 'pk'")]
    pub origin: String,
    #[ts(type = "0n | 1n")]
    pub partial: i64,
    pub columns: Vec<IndexColumn>,
    pub schema: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableSchemaTrigger {
    pub name: String,
    pub sql: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ColumnOriginAndIsRowId {
    pub database: String,
    pub table: String,
    pub column: String,
    pub is_rowid: bool,
}

impl ColumnOriginAndIsRowId {
    pub fn new(is_rowid: bool, column_origin: ColumnOrigin) -> Self {
        Self {
            is_rowid,
            database: column_origin.database,
            table: column_origin.table,
            column: column_origin.column,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TableSchema {
    // InnerTableSchema
    pub schema: Option<String>,
    #[serde(rename = "hasRowIdColumn")]
    pub has_rowid_column: bool,
    pub strict: bool,
    pub columns: Vec<TableSchemaColumn>,
    pub indexes: Vec<TableSchemaIndex>,
    pub triggers: Vec<TableSchemaTrigger>,

    // Optional fields
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub type_: TableType,
    #[serde(rename = "customQuery")]
    pub custom_query: Option<String>,
    #[serde(rename = "columnOrigins")]
    pub column_origins: Option<HashMap<String, ColumnOriginAndIsRowId>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub enum TableType {
    #[serde(rename = "table")]
    Table,
    #[serde(rename = "view")]
    View,
    #[serde(rename = "shadow")]
    Shadow,
    #[serde(rename = "virtual")]
    Virtual,

    /// query_schema() uses this
    #[serde(rename = "custom query")]
    CustomQuery,
    #[serde(rename = "other")]
    Other,
}

impl From<&str> for TableType {
    fn from(value: &str) -> Self {
        match value {
            "table" => Self::Table,
            "view" => Self::View,
            "virtual" => Self::Virtual,
            "shadow" => Self::Shadow,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TableName {
    pub database: Rc<String>,
    pub name: Rc<String>,
    pub type_: TableType,
}
