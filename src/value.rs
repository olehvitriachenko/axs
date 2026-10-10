use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum AxsValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
    Array(Vec<AxsValue>),
    Object(HashMap<String, AxsValue>),
}
