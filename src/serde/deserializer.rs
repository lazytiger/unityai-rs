use std::str::{Chars, FromStr};

use serde::{Deserialize, Deserializer};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

use super::Error;

pub struct UnityDeserializer<'de> {
    tab: usize,
    data: &'de str,
    offset: usize,
    skip_line: bool,
}

impl<'de> UnityDeserializer<'de> {
    fn from_str(data: &'de str) -> UnityDeserializer<'de> {
        UnityDeserializer { data, tab: 0, offset: 0, skip_line: true }
    }

    fn tab_count(&self) -> usize {
        if let Some(count) = self.chars().position(|c| c != '\t') {
            count
        } else {
            self.data.len() - self.offset
        }
    }

    fn skip_header(&mut self) {
        let mut current_eol = 0;
        let pos = self.chars().position(|d| {
            if d == '\n' {
                current_eol += 1;
            } else if d != '\r' {
                current_eol = 0;
            }
            current_eol == 3
        }).expect("skip_header");
        self.skip(pos + 1);
    }

    fn count_until(&self, d: char) -> usize {
        self.chars().position(|c| c == d).expect("skip_until")
    }

    fn chars(&self) -> Chars {
        self.data[self.offset..].chars()
    }

    fn skip(&mut self, count: usize) {
        self.offset += count;
    }

    fn skip_until(&mut self, d: char) {
        let pos = self.count_until(d);
        self.skip(pos + 1);
    }

    fn skip_line(&mut self) {
        self.skip_until('\n');
    }

    fn get_str(&self, len: usize) -> &'de str {
        &self.data[self.offset..self.offset + len]
    }

    fn get_string(&mut self) -> &str {
        let pos = self.chars().position(|c| c == ' ' || c == '\r' || c == '\n').expect("get_string");
        let name = self.get_str(pos);
        if self.chars().nth(pos).unwrap() == ' ' {
            self.skip(pos + 1);
        } else {
            self.skip_line();
        }
        name
    }

    fn get_type(&mut self) -> &str {
        if self.chars().nth(0).unwrap() != '(' {
            //TODO
        } else {
            self.skip(1);
        }
        let pos = self.count_until(')');
        let typ = self.get_str(pos);
        self.skip(pos + 1);
        typ
    }

    fn get_content(&mut self) -> &str {
        let pos = self.chars().position(|c| c == ' ' || c == '\r' || c == '\n').unwrap();
        let content = self.get_str(pos);
        self.skip(pos + 1);
        content
    }

    fn get_content_by<T: FromStr>(&mut self) -> T {
        let content = self.get_content();
        let t = if let Ok(t) = T::from_str(content) {
            t
        } else {
            panic!("parse failed")
        };
        if self.skip_line {
            self.skip_line();
        }
        t
    }

    fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    fn compact(&self) -> bool {
        let mut space_count = 0;
        self.chars().position(|c| {
            if c == ' ' {
                space_count += 1;
            }
            c == '\r' || c == '\n' || c == '('
        });
        space_count == 2
    }

    fn skip_array_header(&mut self) {
        let count = self.count_until(':');
        self.skip(count + 2);
    }
}

pub fn from_str<'a, T: Deserialize<'a>>(data: &'a str) -> super::Result<T> {
    let mut de = UnityDeserializer::from_str(data);
    de.skip_header();
    de.skip_until(')');
    let t = T::deserialize(&mut de)?;
    if de.data.is_empty() {
        Ok(t)
    } else {
        Err(Error {})
    }
}

impl<'de, 'a> Deserializer<'de> for &'a mut UnityDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        //TODO ignored fields go to here
        //1. name content type
        //2. content type
        log::trace!("deserialize_any:{}", self.get_str(10));
        if self.compact() {
            return self.deserialize_identifier(visitor);
        }
        let offset = self.offset;
        let content = self.get_content();
        if content == "" {
            self.deserialize_struct("", &[], visitor)
        } else {
            self.skip(1);
            let typ: String = self.get_type().into();
            self.set_offset(offset);
            match typ.as_str() {
                "SInt64" => self.deserialize_i64(visitor),
                "unsigned int" => self.deserialize_u32(visitor),
                "int" => self.deserialize_i32(visitor),
                "string" => self.deserialize_string(visitor),
                _ => Err(Error {}),
            }
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_bool(self.get_content_by())
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_i8(self.get_content_by())
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_i16(self.get_content_by())
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_i32(self.get_content_by())
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_i64(self.get_content_by())
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_u8(self.get_content_by())
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_u16(self.get_content_by())
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_u32(self.get_content_by())
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_u64(self.get_content_by())
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_f32(self.get_content_by())
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        visitor.visit_f64(self.get_content_by())
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_char")
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        let id = self.get_string();
        log::trace!("id:{}", id);
        visitor.visit_str(id)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        let content = self.get_content();
        let len = content.len();
        let content = content[1..len - 1].into();
        self.skip_line();
        visitor.visit_string(content)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_bytes")
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_byte_buf")
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_option")
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_unit")
    }

    fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_unit_struct")
    }

    fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_newtype_struct")
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        //begin as ' (vector)'
        log::trace!("seq:{}", self.get_str(10));
        self.skip_line(); //current:\t+ size xxx (int)
        self.skip(self.tab_count());
        self.get_string();
        log::trace!("seq:{}", self.get_str(10));
        let count: usize = self.get_content_by();
        //current:data ||
        log::trace!("-------------seq begin:{}---------------", self.get_str(10));
        self.skip(self.tab_count());
        self.get_string();
        let simple = self.get_content() != "";
        log::trace!("-----------seq type:{}", simple);
        self.skip_line = false;
        let access = UnitySeqAccess::new(&mut self, count, simple);
        let ret = visitor.visit_seq(access);
        self.skip_line = true;
        ret
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_tuple")
    }

    fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_tuple_struct")
    }

    fn deserialize_map<V>(mut self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_map")
    }

    fn deserialize_struct<V>(mut self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        self.skip(1);
        let tab = self.tab;
        let id = self.get_string();
        if name != id {
            log::trace!("{} != {}", name, id);
        }
        log::trace!("------begin struct:{}:{}:{}-------", id, name, tab+1);
        self.tab += 1;
        let access = UnityMapAccess::new(&mut self);
        let ret = visitor.visit_map(access);
        self.tab -= 1;
        ret
    }

    fn deserialize_enum<V>(self, name: &'static str, variants: &'static [&'static str], visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_enum")
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        self.deserialize_any(visitor)
    }
}

struct UnityMapAccess<'a, 'de: 'a> {
    tab: usize,
    de: &'a mut UnityDeserializer<'de>,
}

impl<'a, 'de> UnityMapAccess<'a, 'de> {
    fn new(de: &'a mut UnityDeserializer<'de>) -> Self {
        UnityMapAccess { tab: de.tab, de }
    }
}

impl<'a, 'de> MapAccess<'de> for UnityMapAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<<K as DeserializeSeed<'de>>::Value>, Self::Error> where
        K: DeserializeSeed<'de> {
        let tab = self.de.tab_count();
        if tab < self.tab {
            log::trace!("-----end struct:{}----", self.tab);
            return Ok(None);
        }

        log::trace!("next_key_seed:{}", self.de.get_str(10));
        self.de.skip(tab);
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<<V as DeserializeSeed<'de>>::Value, Self::Error> where
        V: DeserializeSeed<'de> {
        log::trace!("next_value_seed:{}", self.de.get_str(10));
        seed.deserialize(&mut *self.de)
    }
}

struct UnitySeqAccess<'a, 'de: 'a> {
    tab: usize,
    de: &'a mut UnityDeserializer<'de>,
    current: usize,
    count: usize,
    simple: bool,
}

impl<'a, 'de> UnitySeqAccess<'a, 'de> {
    fn new(de: &'a mut UnityDeserializer<'de>, count: usize, simple: bool) -> Self {
        UnitySeqAccess {
            tab: de.tab,
            current: 0,
            de,
            simple,
            count,
        }
    }
}

const kArrayMemberColumns: usize = 25;

impl<'a, 'de> SeqAccess<'de> for UnitySeqAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<<T as DeserializeSeed<'de>>::Value>, Self::Error> where
        T: DeserializeSeed<'de> {
        if self.current == self.count {
            log::trace!("------seq end {} {}------", self.current, self.count);
            return Ok(None);
        }

        if self.simple && self.current % kArrayMemberColumns == 0 {
            self.de.skip_array_header();
        }
        self.current += 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
}