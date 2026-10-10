use std::collections::HashMap;

use serde::{Serialize, ser};

use super::{MAX_CONVERSION_DEPTH, invalid, number_error, resource_error};
use crate::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, MAX_TOKENS, valid_key};
use crate::{AxsError, AxsValue, ErrorKind};

type Result<T> = std::result::Result<T, AxsError>;

#[derive(Default)]
pub(super) struct Budget {
    nodes: usize,
    bytes: usize,
}

pub(super) struct Serializer<'a> {
    depth: usize,
    recursion: usize,
    budget: &'a mut Budget,
}

impl<'a> Serializer<'a> {
    pub(super) fn new(budget: &'a mut Budget) -> Self {
        Self {
            depth: 0,
            recursion: 0,
            budget,
        }
    }

    fn tick(&mut self) -> Result<()> {
        if self.recursion > MAX_CONVERSION_DEPTH {
            return Err(resource_error("Maximum Serde conversion depth exceeded"));
        }
        if self.budget.nodes >= MAX_TOKENS {
            return Err(resource_error(
                "Maximum Serde conversion node count exceeded",
            ));
        }
        self.budget.nodes += 1;
        Ok(())
    }

    fn charge_bytes(&mut self, bytes: usize) -> Result<()> {
        if bytes > MAX_INPUT_BYTES - self.budget.bytes {
            return Err(resource_error(
                "Maximum Serde string/key byte budget exceeded",
            ));
        }
        self.budget.bytes += bytes;
        Ok(())
    }

    fn check_block(&self) -> Result<()> {
        if self.depth > MAX_NESTING_DEPTH {
            return Err(AxsError::new(
                ErrorKind::InvalidIndentation,
                "Maximum nesting depth exceeded during Serde serialization",
            ));
        }
        Ok(())
    }

    fn child(&mut self) -> Serializer<'_> {
        Serializer {
            depth: self.depth + 1,
            recursion: self.recursion + 1,
            budget: self.budget,
        }
    }

    fn wrapper(&mut self) -> Serializer<'_> {
        Serializer {
            depth: self.depth,
            recursion: self.recursion + 1,
            budget: self.budget,
        }
    }

    fn key(&mut self, key: &str) -> Result<String> {
        if !valid_key(key) {
            return Err(AxsError::new(
                ErrorKind::InvalidKey,
                format!("Invalid AXS key: '{key}'"),
            ));
        }
        self.charge_bytes(key.len())?;
        Ok(key.to_owned())
    }

    fn seq(mut self, expected: Option<usize>, variant: Option<String>) -> Result<Sequence<'a>> {
        self.tick()?;
        Ok(Sequence {
            serializer: self,
            values: Vec::new(),
            expected,
            variant,
        })
    }

    fn map(mut self, variant: Option<String>) -> Result<Object<'a>> {
        self.tick()?;
        Ok(Object {
            serializer: self,
            values: HashMap::new(),
            pending: None,
            variant,
        })
    }
}

macro_rules! small_integer {
    ($method:ident, $ty:ty) => {
        fn $method(self, value: $ty) -> Result<AxsValue> {
            self.serialize_i64(i64::from(value))
        }
    };
}

impl<'a> ser::Serializer for Serializer<'a> {
    type Ok = AxsValue;
    type Error = AxsError;
    type SerializeSeq = Sequence<'a>;
    type SerializeTuple = Sequence<'a>;
    type SerializeTupleStruct = Sequence<'a>;
    type SerializeTupleVariant = Sequence<'a>;
    type SerializeMap = Object<'a>;
    type SerializeStruct = Object<'a>;
    type SerializeStructVariant = Object<'a>;

    fn serialize_bool(mut self, value: bool) -> Result<AxsValue> {
        self.tick()?;
        Ok(AxsValue::Boolean(value))
    }
    small_integer!(serialize_i8, i8);
    small_integer!(serialize_i16, i16);
    small_integer!(serialize_i32, i32);
    small_integer!(serialize_u8, u8);
    small_integer!(serialize_u16, u16);
    small_integer!(serialize_u32, u32);
    fn serialize_i64(mut self, value: i64) -> Result<AxsValue> {
        self.tick()?;
        Ok(AxsValue::Integer(value))
    }
    fn serialize_u64(self, value: u64) -> Result<AxsValue> {
        let value = i64::try_from(value)
            .map_err(|_| number_error("Unsigned integer is outside the AXS i64 range"))?;
        self.serialize_i64(value)
    }
    fn serialize_i128(self, value: i128) -> Result<AxsValue> {
        let value = i64::try_from(value)
            .map_err(|_| number_error("Integer is outside the AXS i64 range"))?;
        self.serialize_i64(value)
    }
    fn serialize_u128(self, value: u128) -> Result<AxsValue> {
        let value = i64::try_from(value)
            .map_err(|_| number_error("Unsigned integer is outside the AXS i64 range"))?;
        self.serialize_i64(value)
    }
    fn serialize_f32(self, value: f32) -> Result<AxsValue> {
        self.serialize_f64(f64::from(value))
    }
    fn serialize_f64(mut self, value: f64) -> Result<AxsValue> {
        self.tick()?;
        if !value.is_finite() {
            return Err(number_error("Non-finite floats are not supported"));
        }
        Ok(AxsValue::Float(value))
    }
    fn serialize_char(self, value: char) -> Result<AxsValue> {
        self.serialize_str(&value.to_string())
    }
    fn serialize_str(mut self, value: &str) -> Result<AxsValue> {
        self.tick()?;
        self.charge_bytes(value.len())?;
        Ok(AxsValue::String(value.to_owned()))
    }
    fn collect_str<T: std::fmt::Display + ?Sized>(self, value: &T) -> Result<AxsValue> {
        use std::fmt::Write;
        let mut buffer = DisplayBuffer {
            text: String::new(),
            remaining: MAX_INPUT_BYTES - self.budget.bytes,
            exceeded: false,
        };
        if write!(&mut buffer, "{value}").is_err() {
            return Err(if buffer.exceeded {
                resource_error("Maximum Serde string/key byte budget exceeded")
            } else {
                invalid("Display formatting failed during Serde serialization")
            });
        }
        self.serialize_str(&buffer.text)
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<AxsValue> {
        let mut seq = self.seq(Some(value.len()), None)?;
        for byte in value {
            seq.push(byte)?;
        }
        seq.finish()
    }
    fn serialize_none(self) -> Result<AxsValue> {
        self.serialize_unit()
    }
    fn serialize_some<T: Serialize + ?Sized>(mut self, value: &T) -> Result<AxsValue> {
        self.tick()?;
        let value = value.serialize(self.wrapper())?;
        if value == AxsValue::Null {
            return Err(invalid(
                "Some value serializes to null and cannot be distinguished from None",
            ));
        }
        Ok(value)
    }
    fn serialize_unit(mut self) -> Result<AxsValue> {
        self.tick()?;
        Ok(AxsValue::Null)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<AxsValue> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<AxsValue> {
        self.serialize_str(variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        mut self,
        _name: &'static str,
        value: &T,
    ) -> Result<AxsValue> {
        self.tick()?;
        value.serialize(self.wrapper())
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<AxsValue> {
        self.tick()?;
        self.check_block()?;
        let key = self.key(variant)?;
        let value = value.serialize(self.child())?;
        Ok(AxsValue::Object(HashMap::from([(key, value)])))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Sequence<'a>> {
        self.seq(None, None)
    }
    fn serialize_tuple(self, len: usize) -> Result<Sequence<'a>> {
        self.seq(Some(len), None)
    }
    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<Sequence<'a>> {
        self.seq(Some(len), None)
    }
    fn serialize_tuple_variant(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Sequence<'a>> {
        self.tick()?;
        self.check_block()?;
        let key = self.key(variant)?;
        self.depth += 1;
        self.recursion += 1;
        self.seq(Some(len), Some(key))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Object<'a>> {
        self.map(None)
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Object<'a>> {
        self.map(None)
    }
    fn serialize_struct_variant(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Object<'a>> {
        self.tick()?;
        self.check_block()?;
        let key = self.key(variant)?;
        self.depth += 1;
        self.recursion += 1;
        self.map(Some(key))
    }
}

pub(super) struct Sequence<'a> {
    serializer: Serializer<'a>,
    values: Vec<AxsValue>,
    expected: Option<usize>,
    variant: Option<String>,
}
impl Sequence<'_> {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.serializer.check_block()?;
        if self.expected.is_some_and(|len| self.values.len() >= len) {
            return Err(invalid("Too many tuple elements"));
        }
        let index = self.values.len();
        let value = value
            .serialize(self.serializer.child())
            .map_err(|mut error| {
                error.message = format!("at index {index}: {}", error.message);
                error
            })?;
        self.values.push(value);
        Ok(())
    }
    fn finish(self) -> Result<AxsValue> {
        if self.expected.is_some_and(|len| len != self.values.len()) {
            return Err(invalid("Incorrect tuple length"));
        }
        let value = AxsValue::Array(self.values);
        Ok(match self.variant {
            Some(key) => AxsValue::Object(HashMap::from([(key, value)])),
            None => value,
        })
    }
}
macro_rules! sequence_impl {
    ($trait:ident, $element:ident) => {
        impl ser::$trait for Sequence<'_> {
            type Ok = AxsValue;
            type Error = AxsError;
            fn $element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
                self.push(value)
            }
            fn end(self) -> Result<AxsValue> {
                self.finish()
            }
        }
    };
}
sequence_impl!(SerializeSeq, serialize_element);
sequence_impl!(SerializeTuple, serialize_element);
sequence_impl!(SerializeTupleStruct, serialize_field);
sequence_impl!(SerializeTupleVariant, serialize_field);

pub(super) struct Object<'a> {
    serializer: Serializer<'a>,
    values: HashMap<String, AxsValue>,
    pending: Option<String>,
    variant: Option<String>,
}
impl Object<'_> {
    fn insert<T: Serialize + ?Sized>(&mut self, key: String, value: &T) -> Result<()> {
        self.serializer.check_block()?;
        if self.values.contains_key(&key) {
            return Err(AxsError::new(
                ErrorKind::DuplicateKey,
                format!("Duplicate serialized key '{key}'"),
            ));
        }
        let value = value
            .serialize(self.serializer.child())
            .map_err(|mut error| {
                error.message = format!("at field '{key}': {}", error.message);
                error
            })?;
        self.values.insert(key, value);
        Ok(())
    }
    fn finish(self) -> Result<AxsValue> {
        if self.pending.is_some() {
            return Err(invalid("Map key is missing its value"));
        }
        let value = AxsValue::Object(self.values);
        Ok(match self.variant {
            Some(key) => AxsValue::Object(HashMap::from([(key, value)])),
            None => value,
        })
    }
}
impl ser::SerializeMap for Object<'_> {
    type Ok = AxsValue;
    type Error = AxsError;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        if self.pending.is_some() {
            return Err(invalid("Map key is missing its value"));
        }
        self.serializer.check_block()?;
        let key = key.serialize(self.serializer.child())?;
        let AxsValue::String(key) = key else {
            return Err(AxsError::new(
                ErrorKind::InvalidKey,
                "AXS map keys must serialize as strings",
            ));
        };
        if !valid_key(&key) {
            return Err(AxsError::new(
                ErrorKind::InvalidKey,
                format!("Invalid AXS key: '{key}'"),
            ));
        }
        self.pending = Some(key);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let key = self
            .pending
            .take()
            .ok_or_else(|| invalid("Map value has no key"))?;
        self.insert(key, value)
    }
    fn end(self) -> Result<AxsValue> {
        self.finish()
    }
}
macro_rules! object_impl {
    ($trait:ident) => {
        impl ser::$trait for Object<'_> {
            type Ok = AxsValue;
            type Error = AxsError;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<()> {
                let key = self.serializer.key(key)?;
                self.insert(key, value)
            }
            fn end(self) -> Result<AxsValue> {
                self.finish()
            }
        }
    };
}
object_impl!(SerializeStruct);
object_impl!(SerializeStructVariant);

struct DisplayBuffer {
    text: String,
    remaining: usize,
    exceeded: bool,
}
impl std::fmt::Write for DisplayBuffer {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if text.len() > self.remaining {
            self.exceeded = true;
            return Err(std::fmt::Error);
        }
        self.remaining -= text.len();
        self.text.push_str(text);
        Ok(())
    }
}
