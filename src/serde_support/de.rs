use std::collections::HashMap;

use serde::de::{self, DeserializeSeed, Visitor};

use super::{MAX_CONVERSION_DEPTH, invalid, number_error, resource_error};
use crate::{AxsError, AxsValue};

type Result<T> = std::result::Result<T, AxsError>;

pub(super) struct Deserializer {
    value: AxsValue,
    recursion: usize,
}

impl Deserializer {
    pub(super) fn new(value: AxsValue) -> Self {
        Self {
            value,
            recursion: 0,
        }
    }
    fn child(value: AxsValue, recursion: usize) -> Result<Self> {
        if recursion > MAX_CONVERSION_DEPTH {
            return Err(resource_error("Maximum Serde conversion depth exceeded"));
        }
        Ok(Self { value, recursion })
    }
    fn mismatch(&self, expected: &str) -> AxsError {
        let actual = match self.value {
            AxsValue::String(_) => "string",
            AxsValue::Integer(_) => "integer",
            AxsValue::Float(_) => "float",
            AxsValue::Boolean(_) => "boolean",
            AxsValue::Null => "null",
            AxsValue::Array(_) => "array",
            AxsValue::Object(_) => "object",
        };
        invalid(format!("Expected {expected}, found {actual}"))
    }
    fn integer(&self) -> Result<i64> {
        match self.value {
            AxsValue::Integer(value) => Ok(value),
            _ => Err(self.mismatch("integer")),
        }
    }
    fn float64(&self) -> Result<f64> {
        match self.value {
            AxsValue::Float(value) => Ok(value),
            AxsValue::Integer(value) => {
                let converted = value as f64;
                if converted as i128 != i128::from(value) {
                    return Err(number_error("Integer cannot be represented exactly as f64"));
                }
                Ok(converted)
            }
            _ => Err(self.mismatch("float or exactly representable integer")),
        }
    }
    fn sequence<'de, V: Visitor<'de>>(
        self,
        expected: Option<usize>,
        visitor: V,
    ) -> Result<V::Value> {
        let AxsValue::Array(values) = self.value else {
            return Err(self.mismatch("array"));
        };
        if expected.is_some_and(|len| len != values.len()) {
            return Err(invalid("Incorrect tuple length"));
        }
        let mut access = Sequence {
            values: values.into_iter(),
            index: 0,
            recursion: self.recursion + 1,
        };
        let result = visitor.visit_seq(&mut access)?;
        if access.values.len() != 0 {
            return Err(invalid("Sequence visitor did not consume all elements"));
        }
        Ok(result)
    }
    fn object<'de, V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let AxsValue::Object(values) = self.value else {
            return Err(self.mismatch("object"));
        };
        let mut access = Object::new(values, self.recursion + 1);
        let result = visitor.visit_map(&mut access)?;
        if access.pending.is_some() || access.values.len() != 0 {
            return Err(invalid("Map visitor did not consume all entries"));
        }
        Ok(result)
    }
}

macro_rules! integer_method {
    ($method:ident, $visit:ident, $ty:ty) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            let value = <$ty>::try_from(self.integer()?).map_err(|_| {
                number_error(concat!(
                    "Integer is outside the ",
                    stringify!($ty),
                    " range"
                ))
            })?;
            visitor.$visit(value)
        }
    };
}
impl<'de> de::Deserializer<'de> for Deserializer {
    type Error = AxsError;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.value {
            AxsValue::String(value) => visitor.visit_string(value),
            // Serde's buffered Content deserializer may subsequently narrow
            // floats or cast integers without asking this adapter. Require a
            // concrete numeric callback so checked conversion cannot be bypassed.
            AxsValue::Integer(_) | AxsValue::Float(_) => Err(number_error(
                "Numeric deserialize_any is unsupported; use a concrete numeric type and externally tagged enums",
            )),
            AxsValue::Boolean(value) => visitor.visit_bool(value),
            AxsValue::Null => visitor.visit_unit(),
            AxsValue::Array(_) => self.sequence(None, visitor),
            AxsValue::Object(_) => self.object(visitor),
        }
    }
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.value {
            AxsValue::Boolean(value) => visitor.visit_bool(value),
            _ => Err(self.mismatch("boolean")),
        }
    }
    integer_method!(deserialize_i8, visit_i8, i8);
    integer_method!(deserialize_i16, visit_i16, i16);
    integer_method!(deserialize_i32, visit_i32, i32);
    integer_method!(deserialize_i64, visit_i64, i64);
    integer_method!(deserialize_i128, visit_i128, i128);
    integer_method!(deserialize_u8, visit_u8, u8);
    integer_method!(deserialize_u16, visit_u16, u16);
    integer_method!(deserialize_u32, visit_u32, u32);
    integer_method!(deserialize_u64, visit_u64, u64);
    integer_method!(deserialize_u128, visit_u128, u128);
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value = self.float64()?;
        let converted = value as f32;
        if !converted.is_finite() || f64::from(converted).to_bits() != value.to_bits() {
            return Err(number_error(
                "Number cannot be represented exactly as finite f32",
            ));
        }
        visitor.visit_f32(converted)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_f64(self.float64()?)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let AxsValue::String(value) = self.value else {
            return Err(self.mismatch("single-character string"));
        };
        let mut chars = value.chars();
        let character = chars
            .next()
            .ok_or_else(|| invalid("Expected exactly one character, found empty string"))?;
        if chars.next().is_some() {
            return Err(invalid("Expected exactly one Unicode character"));
        }
        visitor.visit_char(character)
    }
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.value {
            AxsValue::String(value) => visitor.visit_string(value),
            _ => Err(self.mismatch("string")),
        }
    }
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let AxsValue::Array(values) = self.value else {
            return Err(self.mismatch("byte array"));
        };
        let mut bytes = Vec::with_capacity(values.len());
        for (index, value) in values.into_iter().enumerate() {
            let AxsValue::Integer(value) = value else {
                return Err(invalid(format!("at index {index}: Expected byte integer")));
            };
            bytes.push(u8::try_from(value).map_err(|_| {
                number_error(format!("at index {index}: Integer is outside the u8 range"))
            })?);
        }
        visitor.visit_byte_buf(bytes)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if matches!(self.value, AxsValue::Null) {
            visitor.visit_none()
        } else {
            visitor.visit_some(Self::child(self.value, self.recursion + 1)?)
        }
    }
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.value {
            AxsValue::Null => visitor.visit_unit(),
            _ => Err(self.mismatch("null")),
        }
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_unit(visitor)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(Self::child(self.value, self.recursion + 1)?)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.sequence(None, visitor)
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        self.sequence(Some(len), visitor)
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        self.sequence(Some(len), visitor)
    }
    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.object(visitor)
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.object(visitor)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let (variant, value) = match self.value {
            AxsValue::String(variant) => (variant, None),
            AxsValue::Object(values) if values.len() == 1 => {
                let (variant, value) = values
                    .into_iter()
                    .next()
                    .ok_or_else(|| invalid("Expected a single enum entry"))?;
                (variant, Some(value))
            }
            _ => return Err(invalid("Expected enum variant string or single-key object")),
        };
        visitor.visit_enum(Enum {
            variant,
            value,
            recursion: self.recursion + 1,
        })
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }
}

struct Sequence {
    values: std::vec::IntoIter<AxsValue>,
    index: usize,
    recursion: usize,
}
impl<'de> de::SeqAccess<'de> for &mut Sequence {
    type Error = AxsError;
    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        let Some(value) = self.values.next() else {
            return Ok(None);
        };
        let index = self.index;
        self.index += 1;
        seed.deserialize(Deserializer::child(value, self.recursion)?)
            .map(Some)
            .map_err(|mut error| {
                error.message = format!("at index {index}: {}", error.message);
                error
            })
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.values.len())
    }
}

struct Object {
    values: std::vec::IntoIter<(String, AxsValue)>,
    pending: Option<(String, AxsValue)>,
    recursion: usize,
}
impl Object {
    fn new(values: HashMap<String, AxsValue>, recursion: usize) -> Self {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_by(|a, b| a.0.cmp(&b.0));
        Self {
            values: values.into_iter(),
            pending: None,
            recursion,
        }
    }
}
impl<'de> de::MapAccess<'de> for &mut Object {
    type Error = AxsError;
    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        if self.pending.is_some() {
            return Err(invalid("Previous map value has not been consumed"));
        }
        let Some((key, value)) = self.values.next() else {
            return Ok(None);
        };
        let decoded = seed.deserialize(Deserializer::child(
            AxsValue::String(key.clone()),
            self.recursion,
        )?)?;
        self.pending = Some((key, value));
        Ok(Some(decoded))
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        let (key, value) = self
            .pending
            .take()
            .ok_or_else(|| invalid("Map value has no key"))?;
        seed.deserialize(Deserializer::child(value, self.recursion)?)
            .map_err(|mut error| {
                error.message = format!("at field '{key}': {}", error.message);
                error
            })
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.values.len())
    }
}

struct Enum {
    variant: String,
    value: Option<AxsValue>,
    recursion: usize,
}
impl<'de> de::EnumAccess<'de> for Enum {
    type Error = AxsError;
    type Variant = Variant;
    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Variant)> {
        let variant = seed.deserialize(Deserializer::child(
            AxsValue::String(self.variant),
            self.recursion,
        )?)?;
        Ok((
            variant,
            Variant {
                value: self.value,
                recursion: self.recursion,
            },
        ))
    }
}
struct Variant {
    value: Option<AxsValue>,
    recursion: usize,
}
impl Variant {
    fn payload(self) -> Result<Deserializer> {
        let value = self
            .value
            .ok_or_else(|| invalid("Data enum variant requires a payload object"))?;
        Deserializer::child(value, self.recursion)
    }
}
impl<'de> de::VariantAccess<'de> for Variant {
    type Error = AxsError;
    fn unit_variant(self) -> Result<()> {
        if self.value.is_some() {
            return Err(invalid("Unit enum variant must be represented as a string"));
        }
        Ok(())
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(self.payload()?)
    }
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        self.payload()?.sequence(Some(len), visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.payload()?.object(visitor)
    }
}
