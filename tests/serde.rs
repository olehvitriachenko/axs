use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

use axs::{AxsError, ErrorKind, from_str, to_string};
use serde::{Deserialize, Serialize, de, ser};

fn roundtrip<T: Serialize + de::DeserializeOwned + PartialEq + Debug>(value: &T) -> String {
    let text = to_string(value).unwrap();
    assert_eq!(&from_str::<T>(&text).unwrap(), value, "{text}");
    assert_eq!(text, to_string(&from_str::<T>(&text).unwrap()).unwrap());
    text
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Profile {
    name: String,
    age: u8,
    active: bool,
    nickname: Option<String>,
    skills: Vec<String>,
    address: Address,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Address {
    city: String,
    zip: String,
}

#[test]
fn simple_and_nested_structs_roundtrip() {
    let profile = Profile {
        name: "Олег 🚀".into(),
        age: 22,
        active: true,
        nickname: None,
        skills: vec!["Rust".into(), "name = value".into()],
        address: Address {
            city: "Київ".into(),
            zip: "00123".into(),
        },
    };
    roundtrip(&profile);
}

#[test]
fn maps_and_nested_sequences_roundtrip() {
    roundtrip(&BTreeMap::from([
        ("one".to_owned(), vec![vec![1i64, 2], vec![]]),
        ("two".to_owned(), vec![]),
    ]));
    roundtrip(&HashMap::from([(
        "map".to_owned(),
        BTreeMap::from([("a".to_owned(), true)]),
    )]));
    roundtrip(&BTreeMap::<String, i32>::new());
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TupleStruct(i32, String);
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Newtype(Vec<i64>);
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Unit;
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Combinations {
    tuple: (i16, bool, String),
    tuple_struct: TupleStruct,
    newtype: Newtype,
    unit: (),
    unit_struct: Unit,
    empty_tuple: (),
    array: [u16; 3],
    character: char,
}

#[test]
fn tuples_newtypes_units_arrays_and_chars_roundtrip() {
    roundtrip(&Combinations {
        tuple: (7, false, "".into()),
        tuple_struct: TupleStruct(-2, "true".into()),
        newtype: Newtype(vec![0, i64::MAX]),
        unit: (),
        unit_struct: Unit,
        empty_tuple: (),
        array: [0, 255, 65535],
        character: '🚀',
    });
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Event {
    Idle,
    #[serde(rename = "renamed")]
    Count(i64),
    Pair(String, bool),
    Record {
        name: String,
        values: Vec<i32>,
    },
    Empty(),
    EmptyRecord {},
    Null(()),
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Events {
    values: Vec<Event>,
}

#[test]
fn all_externally_tagged_enum_variants_roundtrip() {
    let events = Events {
        values: vec![
            Event::Idle,
            Event::Count(8),
            Event::Pair("a=b".into(), true),
            Event::Record {
                name: "null".into(),
                values: vec![1, 2],
            },
            Event::Empty(),
            Event::EmptyRecord {},
            Event::Null(()),
        ],
    };
    roundtrip(&events);
    assert_eq!(to_string(&Event::Count(3)).unwrap(), "renamed = 3\n");
    roundtrip(&Event::Count(3));
    roundtrip(&Event::Pair("x".into(), false));
    roundtrip(&Event::Record {
        name: "n".into(),
        values: vec![],
    });
}

#[test]
fn invalid_enum_shapes_and_unknown_variants_fail() {
    for text in [
        "values:\n  - Unknown",
        "values:\n  - renamed",
        "values:\n  - Idle = null",
        "values:\n  - renamed = true",
        "values:\n  - Pair:\n      - one",
        "values:\n  -\n    renamed = 1\n    extra = 2",
        "values:\n  - {}",
    ] {
        assert!(from_str::<Events>(text).is_err(), "{text}");
    }
}

#[test]
fn root_objects_are_required_for_both_directions() {
    for result in [
        to_string(&42),
        to_string(&vec![1, 2]),
        to_string(&"text"),
        to_string(&()),
        to_string(&None::<i64>),
        to_string(&Event::Idle),
    ] {
        let error = result.unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidValue);
        assert!(error.message.contains("root"));
    }
    assert!(from_str::<i64>("n = 1").is_err());
    assert!(from_str::<Vec<i64>>("n = 1").is_err());
    assert!(from_str::<String>("").is_err());
    assert!(from_str::<()>("").is_err());
    roundtrip(&Some(BTreeMap::from([("n".to_owned(), 1i64)])));
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Wrapped(BTreeMap<String, bool>);
    roundtrip(&Wrapped(BTreeMap::from([("flag".into(), true)])));
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Defaults {
    required: i32,
    optional: Option<String>,
    #[serde(default)]
    values: Vec<i32>,
    #[serde(rename = "display-name", default)]
    name: String,
}

#[test]
fn missing_defaults_null_unknown_and_renamed_fields_follow_serde() {
    let value = from_str::<Defaults>("required = 1\nunknown:\n  - nested = true").unwrap();
    assert_eq!(value.optional, None);
    assert!(value.values.is_empty());
    assert_eq!(value.name, "");
    let null = from_str::<Defaults>("required = 1\noptional = null").unwrap();
    assert_eq!(value, null);
    let renamed =
        from_str::<Defaults>("required = 2\ndisplay-name = Oleh\noptional = Kyiv").unwrap();
    roundtrip(&renamed);
    assert!(
        from_str::<Defaults>("optional = null")
            .unwrap_err()
            .message
            .contains("missing field")
    );
    assert!(from_str::<Defaults>("required = null").is_err());
    assert!(from_str::<Defaults>("required = 1\nvalues = null").is_err());
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Strict {}
    assert!(
        from_str::<Strict>("unexpected = 1")
            .unwrap_err()
            .message
            .contains("unknown field")
    );
}

#[test]
fn null_options_do_not_silently_lose_some() {
    assert!(
        to_string(&BTreeMap::from([("value".to_owned(), Some(()))]))
            .unwrap_err()
            .message
            .contains("cannot be distinguished")
    );
    assert!(
        to_string(&BTreeMap::from([(
            "value".to_owned(),
            Some(None::<String>)
        )]))
        .is_err()
    );
    roundtrip(&BTreeMap::from([(
        "value".to_owned(),
        Some(Some("present".to_owned())),
    )]));
    roundtrip(&BTreeMap::from([("value".to_owned(), None::<()>)]));
}

#[test]
fn string_escaping_and_reserved_literals_use_the_core_serializer() {
    let values = [
        "",
        "true",
        "false",
        "null",
        "[]",
        "{}",
        "22",
        "19.99",
        "9223372036854775808",
        "00123",
        "name = value",
        "a=b=c",
        "# literal",
        "1e10",
        "Олег 🚀",
        "  spaces  ",
        "\"quote\"\\\n\r\t\0",
    ];
    let strings = BTreeMap::from([("values".to_owned(), values.map(str::to_owned).to_vec())]);
    roundtrip(&strings);
}

#[test]
fn signed_and_unsigned_integer_boundaries_roundtrip() {
    roundtrip(&BTreeMap::from([
        ("min".to_owned(), i64::MIN),
        ("max".to_owned(), i64::MAX),
    ]));
    roundtrip(&BTreeMap::from([
        ("max".to_owned(), i64::MAX as u64),
        ("zero".to_owned(), 0u64),
    ]));
    roundtrip(&BTreeMap::from([
        ("max".to_owned(), i128::from(i64::MAX)),
        ("min".to_owned(), i128::from(i64::MIN)),
    ]));
    roundtrip(&BTreeMap::from([("max".to_owned(), i64::MAX as u128)]));
    roundtrip(&BTreeMap::from([("small".to_owned(), i8::MIN)]));
    roundtrip(&BTreeMap::from([("small".to_owned(), u32::MAX)]));
}

#[test]
fn serialization_rejects_integer_overflow_and_nonfinite_floats() {
    for error in [
        to_string(&BTreeMap::from([("n".to_owned(), u64::MAX)])).unwrap_err(),
        to_string(&BTreeMap::from([("n".to_owned(), i128::MAX)])).unwrap_err(),
        to_string(&BTreeMap::from([("n".to_owned(), i128::MIN)])).unwrap_err(),
        to_string(&BTreeMap::from([("n".to_owned(), u128::MAX)])).unwrap_err(),
    ] {
        assert_eq!(error.kind, ErrorKind::InvalidNumber);
        assert!(error.message.contains("field 'n'"));
    }
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            to_string(&BTreeMap::from([("n".to_owned(), number)]))
                .unwrap_err()
                .kind,
            ErrorKind::InvalidNumber
        );
    }
    assert_eq!(
        to_string(&BTreeMap::from([("n".to_owned(), f32::NAN)]))
            .unwrap_err()
            .kind,
        ErrorKind::InvalidNumber
    );
}

#[test]
fn checked_deserialization_rejects_overflow_and_incompatible_types() {
    for result in [
        from_str::<BTreeMap<String, u8>>("n = 256").map(|_| ()),
        from_str::<BTreeMap<String, u8>>("n = -1").map(|_| ()),
        from_str::<BTreeMap<String, i8>>("n = 128").map(|_| ()),
        from_str::<BTreeMap<String, i8>>("n = -129").map(|_| ()),
        from_str::<BTreeMap<String, i16>>("n = 32768").map(|_| ()),
        from_str::<BTreeMap<String, i32>>("n = 2147483648").map(|_| ()),
        from_str::<BTreeMap<String, u32>>("n = 4294967296").map(|_| ()),
    ] {
        assert_eq!(result.unwrap_err().kind, ErrorKind::InvalidNumber);
    }
    for result in [
        from_str::<BTreeMap<String, i64>>("n = 1.0").map(|_| ()),
        from_str::<BTreeMap<String, i64>>("n = \"1\"").map(|_| ()),
        from_str::<BTreeMap<String, String>>("n = 1").map(|_| ()),
        from_str::<BTreeMap<String, bool>>("n = 1").map(|_| ()),
        from_str::<BTreeMap<String, Vec<i64>>>("n = null").map(|_| ()),
        from_str::<BTreeMap<String, char>>("n = two").map(|_| ()),
        from_str::<BTreeMap<String, char>>("n = \"\"").map(|_| ()),
    ] {
        assert_eq!(result.unwrap_err().kind, ErrorKind::InvalidValue);
    }
}

#[test]
fn floats_roundtrip_exact_bits_including_negative_zero() {
    for number in [
        0.0f64,
        -0.0,
        1.5,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
    ] {
        let text = to_string(&BTreeMap::from([("n".to_owned(), number)])).unwrap();
        let values: BTreeMap<String, f64> = from_str(&text).unwrap();
        assert_eq!(values["n"].to_bits(), number.to_bits());
    }
    for number in [
        0.0f32,
        -0.0,
        0.1,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
    ] {
        let text = to_string(&BTreeMap::from([("n".to_owned(), number)])).unwrap();
        let values: BTreeMap<String, f32> = from_str(&text).unwrap();
        assert_eq!(values["n"].to_bits(), number.to_bits());
    }
}

#[test]
fn inexact_integer_to_float_and_float_narrowing_are_errors() {
    for text in [
        "n = 9007199254740993",
        "n = 9223372036854775807",
        "n = -9007199254740993",
    ] {
        assert_eq!(
            from_str::<BTreeMap<String, f64>>(text).unwrap_err().kind,
            ErrorKind::InvalidNumber
        );
    }
    for text in [
        "n = 16777217",
        "n = 0.1",
        "n = 0.00000000000000000000000000000000000000000000000001",
        "n = 340282366920938463463374607431768211456.0",
    ] {
        assert_eq!(
            from_str::<BTreeMap<String, f32>>(text).unwrap_err().kind,
            ErrorKind::InvalidNumber
        );
    }
    assert_eq!(
        from_str::<BTreeMap<String, f64>>("n = 42").unwrap()["n"],
        42.0
    );
    assert_eq!(
        from_str::<BTreeMap<String, f32>>("n = 0.5").unwrap()["n"],
        0.5
    );
    assert_eq!(
        from_str::<BTreeMap<String, f64>>("n = -9223372036854775808").unwrap()["n"],
        i64::MIN as f64
    );
}

#[test]
fn invalid_map_and_renamed_keys_are_errors() {
    assert_eq!(
        to_string(&BTreeMap::from([(1, true)])).unwrap_err().kind,
        ErrorKind::InvalidKey
    );
    for key in ["", "1key", "bad key", "a=b", "🚀"] {
        assert_eq!(
            to_string(&BTreeMap::from([(key, true)])).unwrap_err().kind,
            ErrorKind::InvalidKey
        );
    }
    assert!(from_str::<BTreeMap<i32, bool>>("one = true").is_err());
    #[derive(Serialize)]
    struct BadField {
        #[serde(rename = "bad key")]
        value: bool,
    }
    assert_eq!(
        to_string(&BadField { value: true }).unwrap_err().kind,
        ErrorKind::InvalidKey
    );
    #[derive(Serialize)]
    enum BadVariant {
        #[serde(rename = "bad key")]
        Data(i64),
    }
    assert_eq!(
        to_string(&BadVariant::Data(1)).unwrap_err().kind,
        ErrorKind::InvalidKey
    );
}

#[test]
fn parser_errors_keep_spans_and_conversion_errors_have_context() {
    let error = from_str::<Profile>("age = 9223372036854775808").unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidNumber);
    assert_eq!(error.span.unwrap().column, 7);
    let error = from_str::<BTreeMap<String, Vec<u8>>>("values:\n  - 1\n  - 256").unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidNumber);
    assert!(error.message.contains("field 'values'"));
    assert!(error.message.contains("index 1"));
    assert!(error.span.is_none());
}

#[derive(Debug)]
struct CustomError;
impl Serialize for CustomError {
    fn serialize<S: ser::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(ser::Error::custom("custom serialization failure"))
    }
}
impl<'de> Deserialize<'de> for CustomError {
    fn deserialize<D: de::Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        Err(de::Error::custom("custom deserialization failure"))
    }
}
#[test]
fn custom_errors_propagate_as_axs_errors() {
    let error = to_string(&BTreeMap::from([("field".to_owned(), CustomError)])).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidValue);
    assert!(error.message.contains("custom serialization failure"));
    let error = <AxsError as de::Error>::custom("custom deserialization failure");
    assert_eq!(error.kind, ErrorKind::InvalidValue);
    let error = from_str::<BTreeMap<String, CustomError>>("field = value").unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidValue);
    assert!(
        error
            .message
            .contains("field 'field': custom deserialization failure")
    );
}

struct BadMap(u8);
impl Serialize for BadMap {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(usize::MAX))?;
        match self.0 {
            0 => {
                map.serialize_entry("same", &1)?;
                map.serialize_entry("same", &2)?;
            }
            1 => {
                map.serialize_value(&1)?;
            }
            2 => {
                map.serialize_key("dangling")?;
            }
            _ => {
                map.serialize_key("first")?;
                map.serialize_key("second")?;
            }
        }
        map.end()
    }
}
#[test]
fn malformed_custom_map_protocols_return_errors() {
    for mode in 0..4 {
        assert!(to_string(&BadMap(mode)).is_err());
    }
    assert_eq!(
        to_string(&BadMap(0)).unwrap_err().kind,
        ErrorKind::DuplicateKey
    );
}

struct Layers(usize);
impl Serialize for Layers {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 == 0 {
            return serializer.serialize_unit();
        }
        use ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("child", &Layers(self.0 - 1))?;
        map.end()
    }
}
#[test]
fn collection_depth_is_checked_while_building_the_tree() {
    let text = to_string(&Layers(129)).unwrap();
    assert!(axs::parse(&text).is_ok());
    assert_eq!(
        to_string(&Layers(130)).unwrap_err().kind,
        ErrorKind::InvalidIndentation
    );
    assert_eq!(
        to_string(&Layers(10_000)).unwrap_err().kind,
        ErrorKind::InvalidIndentation
    );
    let mut source = String::new();
    for depth in 0..=129 {
        source.push_str(&"  ".repeat(depth));
        source.push_str("child:\n");
    }
    assert_eq!(
        from_str::<BTreeMap<String, String>>(&source)
            .unwrap_err()
            .kind,
        ErrorKind::InvalidIndentation
    );
}

struct Transparent(usize);
impl Serialize for Transparent {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 == 0 {
            serializer.serialize_unit()
        } else {
            serializer.serialize_newtype_struct("Transparent", &Transparent(self.0 - 1))
        }
    }
}
#[test]
fn transparent_wrapper_recursion_is_bounded() {
    assert_eq!(
        to_string(&BTreeMap::from([("value".to_owned(), Transparent(10_000))]))
            .unwrap_err()
            .kind,
        ErrorKind::SyntaxError
    );
}

struct NullSequence(usize);
impl Serialize for NullSequence {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(None)?;
        for _ in 0..self.0 {
            sequence.serialize_element(&())?;
        }
        sequence.end()
    }
}
#[test]
fn byte_token_and_conversion_budgets_are_enforced() {
    use axs::lexer::{MAX_INPUT_BYTES, MAX_TOKENS};
    let value = "x".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(
        to_string(&BTreeMap::from([("value".to_owned(), value)]))
            .unwrap_err()
            .kind,
        ErrorKind::SyntaxError
    );
    let escaped = "\0".repeat(MAX_INPUT_BYTES / 2);
    assert_eq!(
        to_string(&BTreeMap::from([("value".to_owned(), escaped)]))
            .unwrap_err()
            .kind,
        ErrorKind::SyntaxError
    );
    assert_eq!(
        to_string(&BTreeMap::from([(
            "value".to_owned(),
            NullSequence(MAX_TOKENS / 2)
        )]))
        .unwrap_err()
        .kind,
        ErrorKind::SyntaxError
    );
    assert_eq!(
        to_string(&BTreeMap::from([(
            "value".to_owned(),
            NullSequence(MAX_TOKENS)
        )]))
        .unwrap_err()
        .kind,
        ErrorKind::SyntaxError
    );
    assert_eq!(
        from_str::<BTreeMap<String, bool>>(&"#".repeat(MAX_INPUT_BYTES + 1))
            .unwrap_err()
            .kind,
        ErrorKind::SyntaxError
    );
}

#[test]
fn tuple_lengths_are_checked() {
    for source in ["v = []", "v:\n  - 1", "v:\n  - 1\n  - true\n  - extra"] {
        assert!(from_str::<BTreeMap<String, (i32, bool)>>(source).is_err());
    }
    roundtrip(&BTreeMap::from([("v".to_owned(), (1i32, true))]));
}

#[test]
fn property_numeric_and_string_roundtrips_are_reproducible() {
    let mut seed = 0x4158_535f_5345_5244u64;
    for _ in 0..2_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let values = BTreeMap::from([
            ("min".to_owned(), seed as i64),
            ("max".to_owned(), !(seed as i64)),
        ]);
        roundtrip(&values);
        let float = f64::from_bits(seed);
        if float.is_finite() {
            let text = to_string(&BTreeMap::from([("n".to_owned(), float)])).unwrap();
            assert_eq!(
                from_str::<BTreeMap<String, f64>>(&text).unwrap()["n"].to_bits(),
                float.to_bits()
            );
        }
        let float = f32::from_bits(seed as u32);
        if float.is_finite() {
            let text = to_string(&BTreeMap::from([("n".to_owned(), float)])).unwrap();
            assert_eq!(
                from_str::<BTreeMap<String, f32>>(&text).unwrap()["n"].to_bits(),
                float.to_bits()
            );
        }
        roundtrip(&BTreeMap::from([(
            "strings".to_owned(),
            vec![
                format!("name = {seed}"),
                format!("{seed}"),
                format!("🚀\n{seed}\0"),
            ],
        )]));
    }
}

#[derive(Debug, PartialEq)]
struct Bytes(Vec<u8>);
impl Serialize for Bytes {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}
impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BytesVisitor;
        impl de::Visitor<'_> for BytesVisitor {
            type Value = Bytes;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a byte array")
            }
            fn visit_byte_buf<E: de::Error>(self, bytes: Vec<u8>) -> Result<Bytes, E> {
                Ok(Bytes(bytes))
            }
        }
        deserializer.deserialize_byte_buf(BytesVisitor)
    }
}
#[test]
fn explicit_bytes_use_checked_integer_arrays() {
    roundtrip(&BTreeMap::from([(
        "bytes".to_owned(),
        Bytes(vec![0, 1, 127, 255]),
    )]));
    roundtrip(&BTreeMap::from([("bytes".to_owned(), Bytes(vec![]))]));
    for text in ["bytes:\n  - -1", "bytes:\n  - 256"] {
        assert_eq!(
            from_str::<BTreeMap<String, Bytes>>(text).unwrap_err().kind,
            ErrorKind::InvalidNumber
        );
    }
    assert_eq!(
        from_str::<BTreeMap<String, Bytes>>("bytes:\n  - 1.0")
            .unwrap_err()
            .kind,
        ErrorKind::InvalidValue
    );
}

#[test]
fn buffered_numeric_enums_cannot_bypass_checked_conversion() {
    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    enum Buffered {
        Float(f32),
    }
    let result = from_str::<BTreeMap<String, Buffered>>("n = 0.1");
    assert!(result.is_err());
    // Exercise the derived payload so Clippy verifies the type without dead fields.
    let Buffered::Float(number) = Buffered::Float(0.5);
    assert_eq!(number, 0.5);
    #[derive(Debug, Deserialize)]
    #[serde(tag = "kind")]
    enum Tagged {
        Data { value: f32 },
    }
    assert!(from_str::<Tagged>("kind = Data\nvalue = 0.1").is_err());
    let Tagged::Data { value } = Tagged::Data { value: 0.5 };
    assert_eq!(value, 0.5);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DeserializeLoop;
impl<'de> Deserialize<'de> for DeserializeLoop {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct LoopVisitor;
        impl<'de> de::Visitor<'de> for LoopVisitor {
            type Value = DeserializeLoop;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a wrapper")
            }
            fn visit_newtype_struct<D: de::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                DeserializeLoop::deserialize(deserializer)
            }
        }
        deserializer.deserialize_newtype_struct("Loop", LoopVisitor)
    }
}
#[test]
fn deserialization_wrapper_recursion_is_bounded() {
    assert_eq!(
        from_str::<DeserializeLoop>("").unwrap_err().kind,
        ErrorKind::SyntaxError
    );
    assert_eq!(
        from_str::<BTreeMap<DeserializeLoop, String>>("key = value")
            .unwrap_err()
            .kind,
        ErrorKind::SyntaxError
    );
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Key(String);
#[test]
fn transparent_string_keys_and_char_keys_are_supported() {
    roundtrip(&BTreeMap::from([(Key("key".into()), true)]));
    roundtrip(&BTreeMap::from([('x', true)]));
}

struct DisplayValue(usize);
impl std::fmt::Display for DisplayValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for _ in 0..self.0 {
            f.write_str("text")?;
        }
        Ok(())
    }
}
impl Serialize for DisplayValue {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
#[test]
fn collect_str_respects_the_intermediate_byte_budget() {
    assert_eq!(
        to_string(&BTreeMap::from([("display", DisplayValue(3))])).unwrap(),
        "display = texttexttext\n"
    );
    assert_eq!(
        to_string(&BTreeMap::from([(
            "display",
            DisplayValue(axs::lexer::MAX_INPUT_BYTES)
        )]))
        .unwrap_err()
        .kind,
        ErrorKind::SyntaxError
    );
}

#[test]
fn malformed_inputs_return_errors_through_the_public_serde_api() {
    for source in [
        "x = \"",
        "x = \"\\q\"",
        "x = 9223372036854775808",
        "  x = 1",
        "x:\n   y = 1",
        "name = a\nname = b",
    ] {
        let error = from_str::<BTreeMap<String, String>>(source).unwrap_err();
        assert!(error.span.is_some());
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Node {
    child: Option<Box<Node>>,
}
#[test]
fn nested_option_structs_reach_the_existing_128_level_boundary() {
    let mut node = Node { child: None };
    for _ in 0..128 {
        node = Node {
            child: Some(Box::new(node)),
        };
    }
    roundtrip(&node);
    node = Node {
        child: Some(Box::new(node)),
    };
    assert_eq!(
        to_string(&node).unwrap_err().kind,
        ErrorKind::InvalidIndentation
    );
}

#[derive(Debug)]
struct PartialSequence;
impl<'de> Deserialize<'de> for PartialSequence {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PartialVisitor;
        impl<'de> de::Visitor<'de> for PartialVisitor {
            type Value = PartialSequence;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let _: Option<i64> = sequence.next_element()?;
                Ok(PartialSequence)
            }
        }
        deserializer.deserialize_seq(PartialVisitor)
    }
}
#[test]
fn partial_custom_visitors_cannot_silently_discard_sequence_elements() {
    let error = from_str::<BTreeMap<String, PartialSequence>>("v:\n  - 1\n  - 2").unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidValue);
    assert!(error.message.contains("did not consume"));
}
