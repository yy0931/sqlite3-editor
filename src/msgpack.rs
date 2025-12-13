use std::collections::HashMap;
use std::io::Write;

use rusqlite::types::ValueRef;
use serde::Serialize;

use crate::utf8_extractor::into_utf8_lossy;
use crate::utf8_extractor::InvalidUTF8;

const ENCODING_ERROR: &str = "Failed to encode a MessagePack";

/// Represents a MessagePack for `Record<string, any>`.
///
/// # Example
///
/// ```
/// let data = Data {
///     a: 42,
///     b: "hello".into(),
/// };
/// let mut map = MessagePackRecordBuilder::new();
/// map.insert("key1", &123);
/// map.insert("key2", &data);
/// let message_pack = map.into_vec();
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq, derive_new::new)]
pub struct MessagePackRecord {
    #[new(default)]
    entries: HashMap<String, Vec<u8>>,
    #[new(default)]
    keys_in_insertion_order: Vec<String>,
}

impl MessagePackRecord {
    /// Inserts a key–value pair into the record, where the value has already been encoded as MessagePack.
    pub fn insert_message_pack(&mut self, key: impl Into<String>, value: Vec<u8>) {
        use std::collections::hash_map::Entry;
        match self.entries.entry(key.into()) {
            Entry::Occupied(mut entry) => {
                entry.insert(value); // update existing entry
            }
            Entry::Vacant(entry) => {
                self.keys_in_insertion_order.push(entry.key().clone());
                entry.insert(value); // add a new entry
            }
        }
    }

    /// Inserts a key–value pair into the record, encoding the value as MessagePack.
    pub fn insert_value<T: Serialize>(&mut self, key: impl Into<String>, value: &T) {
        self.insert_message_pack(key, rmp_serde::to_vec_named(value).expect(ENCODING_ERROR));
    }

    /// Writes the MessagePack using the given writer.
    pub fn write_to(&self, writer: &mut impl Write) {
        rmp::encode::write_map_len(writer, self.entries.len() as u32).expect(ENCODING_ERROR);

        for k in &self.keys_in_insertion_order {
            rmp::encode::write_str(writer, k).expect(ENCODING_ERROR);

            // NOTE: We could encode values directly to the writer, but that would add complexity due to lifetimes and wouldn't be worth it.
            writer.write_all(&self.entries[k]).expect(ENCODING_ERROR);
        }
    }

    /// Encodes the MessagePack as a Vec<u8>.
    #[allow(unused)]
    pub fn to_vec(&self) -> Vec<u8> {
        let mut writer = vec![];
        self.write_to(&mut writer);
        writer
    }
}

/// Represents a MessagePack for `any[]`.
///
/// # Example
///
/// ```
/// let data = Data { a: 42, b: "hello".into() };
/// let mut arr = MessagePackArrayBuilder::new();
/// arr.push_value(&123);
/// arr.push_value(&data);
/// let message_pack = arr.to_vec();
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq, derive_new::new)]
pub struct MessagePackArray {
    #[new(default)]
    entries: Vec<Vec<u8>>,
}

impl MessagePackArray {
    /// Pushes a value already encoded as MessagePack.
    pub fn push_message_pack(&mut self, value: Vec<u8>) {
        self.entries.push(value);
    }

    /// Pushes a value, encoding it as MessagePack.
    #[allow(unused)]
    pub fn push_value<T: Serialize>(&mut self, value: &T) {
        self.push_message_pack(rmp_serde::to_vec_named(value).expect(ENCODING_ERROR));
    }

    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Reduces memory usage by shrinking the inner vectors.
    pub fn shrink_to_fit(&mut self) {
        for entry in &mut self.entries {
            entry.shrink_to_fit();
        }
        self.entries.shrink_to_fit();
    }

    /// Writes the MessagePack using the given writer.
    pub fn write_to(&self, writer: &mut impl Write) {
        rmp::encode::write_array_len(writer, self.entries.len() as u32).expect(ENCODING_ERROR);

        for v in &self.entries {
            // NOTE: Encoding values directly to `writer` would complicate lifetimes.
            writer.write_all(v).expect(ENCODING_ERROR);
        }
    }

    /// Encodes the MessagePack as a Vec<u8>.
    pub fn to_vec(&self) -> Vec<u8> {
        let mut writer = vec![];
        self.write_to(&mut writer);
        writer
    }

    /// Returns a reference to an element encoded as a MessagePack.
    pub fn get_as_msgpack(&self, index: usize) -> Option<&[u8]> {
        self.entries.get(index).map(|v| v.as_ref())
    }

    /// Returns the total amount of memory used by this instance in bytes.
    pub fn memory_usage(&self) -> usize {
        use std::mem::size_of;
        let mut total = size_of::<Self>();
        total += size_of::<Vec<u8>>() * self.entries.capacity();
        total += self.entries.iter().map(|v| v.capacity()).sum::<usize>();
        total
    }
}

/// Encodes rusqlite::types::value_ref as MessagePack.
pub fn encode_value_ref_into_msgpack<F: FnMut(InvalidUTF8)>(value: ValueRef, on_invalid_utf8: F) -> Vec<u8> {
    let mut wr = vec![];
    match value {
        ValueRef::Null => {
            rmp::encode::write_nil(&mut wr).expect(ENCODING_ERROR);
        }
        ValueRef::Integer(v) => {
            rmp::encode::write_sint(&mut wr, v).expect(ENCODING_ERROR);
        }
        ValueRef::Text(v) => {
            rmp::encode::write_str(&mut wr, &into_utf8_lossy(v, on_invalid_utf8)).expect(ENCODING_ERROR);
        }
        ValueRef::Real(v) => {
            rmp::encode::write_f64(&mut wr, v).expect(ENCODING_ERROR);
        }
        ValueRef::Blob(v) => {
            rmp::encode::write_bin(&mut wr, v).expect(ENCODING_ERROR);
        }
    }
    wr
}

/// Decodes a MessagePack to JSON.
#[allow(unused)]
pub fn decode_msgpack_into_json(slice: impl AsRef<[u8]>) -> String {
    read_msgpack_into_json(&mut std::io::Cursor::new(slice))
}

/// Reads a MessagePack Read and decodes it to a JSON.
pub fn read_msgpack_into_json(r: &mut (impl std::io::Read + std::io::Seek)) -> String {
    r.rewind().expect("Failed to rewind the reader.");
    let mut json = vec![];
    match serde_transcode::transcode(&mut rmp_serde::Deserializer::new(r), &mut serde_json::Serializer::new(&mut json)) {
        Ok(_) => String::from_utf8_lossy(&json).to_string(),
        Err(_) => "<Failed to serialize as a JSON>".to_owned(),
    }
}

/// Explains a MessagePack data byte-by-byte.
///
/// # Examples
/// ```
/// assert_eq!(explain_msgpack([0x92, 0xC0, 0xC2]), "fixarray(2) nil false");
/// assert_eq!(explain_msgpack([0x01, 0x7F]), "positive_fixint(1) positive_fixint(127)");
/// assert_eq!(explain_msgpack([0xE0, 0xFF]), "negative_fixint(-32) negative_fixint(-1)");
/// assert_eq!(explain_msgpack([0xA3, b'f', b'o', b'o']), "fixstr(3) 102 111 111");
/// ```
#[allow(unused)]
pub fn explain_msgpack(slice: impl AsRef<[u8]>) -> String {
    let mut out = Vec::new();
    let mut data_rem: u8 = 0;

    for &b in slice.as_ref() {
        if data_rem > 0 {
            out.push(format!("{b}"));
            data_rem -= 1;
            continue;
        }

        #[rustfmt::skip]
        out.push(match b {
            0x00..=0x7F => format!("positive_fixint({})", b),
            0x80..=0x8F => format!("fixmap({})", b & 0x0F),
            0x90..=0x9F => format!("fixarray({})", b & 0x0F),
            0xA0..=0xBF => { data_rem = b & 0x1F; format!("fixstr({})", data_rem) }
            0xC0 => "nil".to_string(),
            0xC1 => "reserved".to_string(), // never used
            0xC2 => "false".to_string(),
            0xC3 => "true".to_string(),
            0xC4 => { unimplemented!("bin8") }
            0xC5 => { unimplemented!("bin16") }
            0xC6 => { unimplemented!("bin32") }
            0xC7 => { unimplemented!("ext8") }
            0xC8 => { unimplemented!("ext16") }
            0xC9 => { unimplemented!("ext32") }
            0xCA => { data_rem = 4;"float32".to_string() }
            0xCB => { data_rem = 8;"float64".to_string() }
            0xCC => { data_rem = 1; "uint8".to_string() }
            0xCD => { data_rem = 2; "uint16".to_string() }
            0xCE => { data_rem = 4; "uint32".to_string() }
            0xCF => { data_rem = 8; "uint64".to_string() }
            0xD0 => { data_rem = 1; "int8".to_string() }
            0xD1 => { data_rem = 2; "int16".to_string() }
            0xD2 => { data_rem = 4; "int32".to_string() }
            0xD3 => { data_rem = 8; "int64".to_string() }
            0xD4 => { unimplemented!("fixext1") }
            0xD5 => { unimplemented!("fixext2") }
            0xD6 => { unimplemented!("fixext4") }
            0xD7 => { unimplemented!("fixext8") }
            0xD8 => { unimplemented!("fixext16") }
            0xD9 => { unimplemented!("str8") }
            0xDA => { unimplemented!("str16") }
            0xDB => { unimplemented!("str32") }
            0xDC => { data_rem = 2; "array16".to_string() }
            0xDD => { data_rem = 2; "array32".to_string() }
            0xDE => { data_rem = 2; "map16".to_string() }
            0xDF => { data_rem = 4; "map32".to_string() }
            0xE0..=0xFF => { format!("negative_fixint({})", (b as i8) as i32) }
        });
    }

    out.join(" ")
}

#[cfg(test)]
mod tests {
    use crate::msgpack::explain_msgpack;

    use super::decode_msgpack_into_json;
    use super::MessagePackArray;
    use super::MessagePackRecord;
    use serde::Deserialize;
    use serde::Serialize;

    #[test]
    fn test_explain_msgpack() {
        assert_eq!(explain_msgpack([0x92, 0xC0, 0xC2]), "fixarray(2) nil false");
        assert_eq!(explain_msgpack([0x01, 0x7F]), "positive_fixint(1) positive_fixint(127)");
        assert_eq!(explain_msgpack([0xE0, 0xFF]), "negative_fixint(-32) negative_fixint(-1)");
        assert_eq!(explain_msgpack([0xA3, b'f', b'o', b'o']), "fixstr(3) 102 111 111");
    }

    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct Data {
        a: i32,
        b: String,
    }

    #[test]
    fn test_message_pack_record_builder_1() {
        let data = Data { a: 42, b: "hello".into() };
        let mut map = MessagePackRecord::new();
        map.insert_value("A", &123);
        map.insert_value("B", &data);
        map.insert_value("C", &123.0);
        map.insert_value("D", &None::<String>);
        let buf = map.to_vec();
        let expected = r"
            fixmap(4)
                fixstr(1) 65  positive_fixint(123) 
                fixstr(1) 66  fixmap(2)
                                fixstr(1) 97  positive_fixint(42) 
                                fixstr(1) 98  fixstr(5) 104 101 108 108 111 
                fixstr(1) 67  float64 64 94 192 0 0 0 0 0
                fixstr(1) 68  nil";
        assert_eq!(
            explain_msgpack(&buf),
            regex::Regex::new(r"\s{2,}").unwrap().replace_all(expected, " ").trim()
        );
        assert_eq!(
            decode_msgpack_into_json(buf),
            r#"{"A":123,"B":{"a":42,"b":"hello"},"C":123.0,"D":null}"#
        );
    }

    #[test]
    fn test_message_pack_record_builder_retain_insertion_order() {
        let mut map = MessagePackRecord::new();
        map.insert_value("key2", &1);
        map.insert_value("key3", &2);
        map.insert_value("key1", &3);
        let buf = map.to_vec();
        let expected = r"
            fixmap(3)
                fixstr(4) 107 101 121 50  positive_fixint(1)
                fixstr(4) 107 101 121 51  positive_fixint(2)
                fixstr(4) 107 101 121 49  positive_fixint(3)";
        assert_eq!(
            explain_msgpack(&buf),
            regex::Regex::new(r"\s{2,}").unwrap().replace_all(expected, " ").trim()
        );
    }

    #[test]
    fn test_message_pack_record_builder_update_existing_key() {
        let mut map = MessagePackRecord::new();
        map.insert_value("key2", &1);
        map.insert_value("key3", &2);
        map.insert_value("key1", &3);
        map.insert_value("key3", &20);
        let buf = map.to_vec();
        let expected = r"
            fixmap(3)
                fixstr(4) 107 101 121 50  positive_fixint(1)
                fixstr(4) 107 101 121 51  positive_fixint(20)
                fixstr(4) 107 101 121 49  positive_fixint(3)";
        assert_eq!(
            explain_msgpack(&buf),
            regex::Regex::new(r"\s{2,}").unwrap().replace_all(expected, " ").trim()
        );
    }

    #[test]
    fn test_message_pack_array_builder_1() {
        #[derive(Debug, PartialEq, Deserialize, Serialize)]
        struct Data {
            a: i32,
            b: String,
        }

        let data = Data { a: 42, b: "hello".into() };
        let mut arr = MessagePackArray::new();
        arr.push_value(&123);
        arr.push_value(&data);
        arr.push_value(&123.0f64);
        arr.push_value(&None::<String>);

        let buf = arr.to_vec();

        // fixarray(4) [123, {a:42,b:"hello"}, 123.0, null]
        let expected = r"
            fixarray(4)
                positive_fixint(123) 
                fixmap(2)
                    fixstr(1) 97  positive_fixint(42) 
                    fixstr(1) 98  fixstr(5) 104 101 108 108 111 
                float64 64 94 192 0 0 0 0 0
                nil";

        assert_eq!(
            explain_msgpack(&buf),
            regex::Regex::new(r"\s{2,}").unwrap().replace_all(expected, " ").trim()
        );

        assert_eq!(decode_msgpack_into_json(&buf), r#"[123,{"a":42,"b":"hello"},123.0,null]"#);
    }
}
