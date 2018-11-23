use chrono::NaiveDateTime;

use schema::key_values;

#[derive(Debug, Queryable, Clone)]
pub struct KeyValue {
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Insertable, Clone)]
#[table_name = "key_values"]
pub struct NewKeyValue {
    pub key: String,
    pub value: serde_json::Value,
}
