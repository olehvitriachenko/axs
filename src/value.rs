pub use indexmap::IndexMap;

/// An AXS object: unique string keys in insertion (source) order.
///
/// Equality ignores order, so two objects with the same entries are equal as
/// data. Serializers always write entries in this map's order.
pub type AxsObject = IndexMap<String, AxsValue>;

#[derive(Debug, Clone, PartialEq)]
pub enum AxsValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
    Array(Vec<AxsValue>),
    Object(AxsObject),
}

/// Recursively sorts every object's keys lexicographically, for callers that
/// want order-independent output (`axs fmt --sort-keys`).
pub fn sort_keys(object: &mut AxsObject) {
    object.sort_keys();
    for value in object.values_mut() {
        sort_value_keys(value);
    }
}

fn sort_value_keys(value: &mut AxsValue) {
    match value {
        AxsValue::Object(object) => sort_keys(object),
        AxsValue::Array(items) => items.iter_mut().for_each(sort_value_keys),
        _ => {}
    }
}
