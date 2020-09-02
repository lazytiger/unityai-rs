use std::str::{Chars, FromStr};

use regex::Regex;
use serde::{Deserialize, Deserializer};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

use super::UnityDeError;

#[derive(Copy, Clone)]
enum DeStatus {
    MultipleElement,
    SingleElement,
    StructKey,
    StructValue,
    Invalid,
}

pub struct UnityDeserializer<'de> {
    tab: usize,
    data: &'de str,
    offset: usize,
    status: Vec<DeStatus>,
    regex: Regex,
}

impl<'de> UnityDeserializer<'de> {
    fn from_str(data: &'de str) -> UnityDeserializer<'de> {
        let mut status = Vec::new();
        status.push(DeStatus::Invalid);
        let regex = Regex::new(r"data \([0-9a-zA-Z ]+\) #[0-9]+:").unwrap();
        UnityDeserializer { data, tab: 0, offset: 0, status, regex }
    }

    fn current_status(&self) -> DeStatus {
        *self.status.last().unwrap()
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
        if let DeStatus::MultipleElement = self.current_status() {} else {
            self.skip_until('\n');
        }
    }

    fn get_str(&mut self, len: usize) -> &'de str {
        let ret = &self.data[self.offset..self.offset + len];
        self.skip(len);
        ret
    }

    fn peek_str(&self, len: usize) -> &'de str {
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

    fn peek_type(&mut self) -> super::Result<&str> {
        let line = self.peek_line();
        let (bgn, _) = line.char_indices().rfind(|(_, c)| *c == '(').ok_or_else(|| UnityDeError {})?;
        let end = line[bgn + 1..].chars().position(|c| c == ')').ok_or_else(|| UnityDeError {})?;
        Ok(&line[bgn + 1..bgn + end + 1])
    }

    fn get_identifier(&mut self) -> super::Result<&str> {
        let pos = self.chars().position(|c| !c.is_ascii_alphanumeric() && c != '_').ok_or_else(|| UnityDeError {})?;
        Ok(self.get_str(pos))
    }

    fn get_char(&mut self) -> char {
        self.chars().nth(0).unwrap()
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
        self.skip_line();
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
        //log::trace!("skip array header");
        let count = self.count_until(':');
        self.skip(count + 2);
    }

    fn peek_line(&self) -> &str {
        let pos = self.chars().position(|c| c == '\r' || c == '\n').unwrap();
        self.peek_str(pos)
    }

    fn is_seq_multi(&self) -> bool {
        self.regex.is_match(self.peek_line())
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
        log::error!("trailing data found");
        Err(UnityDeError {})
    }
}

impl<'de, 'a> Deserializer<'de> for &'a mut UnityDeserializer<'de> {
    type Error = UnityDeError;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        //TODO ignore unknown format
        //There are two types of line.
        //1. name content type
        //2. content type
        match self.current_status() {
            DeStatus::StructKey => {
                log::trace!("deserialize_any:StructKey, input='{}'", self.peek_line());
                return self.deserialize_identifier(visitor);
            }
            DeStatus::StructValue | DeStatus::SingleElement => {
                log::trace!("deserialize_any:StructValue, input='{}'", self.peek_line());
            }
            DeStatus::MultipleElement => {
                log::trace!("deserialize_any:PrimitiveElement, input='{}'", self.peek_line());
            }
            DeStatus::Invalid => {
                unreachable!("invalid status")
            }
        }
        let offset = self.offset;
        self.get_content();
        let typ: String = self.peek_type()?.into();
        log::trace!("found type:{}", typ);
        self.offset = offset;
        match typ.as_str() {
            "vector" => self.deserialize_seq(visitor),
            "SInt64" => self.deserialize_i64(visitor),
            "unsigned int" => self.deserialize_u32(visitor),
            "int" => self.deserialize_i32(visitor),
            "string" => self.deserialize_string(visitor),
            "UInt8" => self.deserialize_u8(visitor),
            "float" => self.deserialize_f32(visitor),
            "Vector3f" => self.deserialize_str(visitor),
            _ => self.deserialize_struct("", &[], visitor),
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
        let id = self.peek_line();
        let ret = visitor.visit_str(id);
        self.skip_line();
        ret
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
        log::trace!("deserialize_seq:input='{}'", self.peek_line());
        self.skip_line();

        //current:\t+ size xxx (int)
        log::trace!("deserialize_seq:input='{}'", self.peek_line());
        self.skip(self.tab_count());
        self.get_string();
        //57 (int)
        log::trace!("deserialize_seq:input='{}'", self.peek_line());
        let count: usize = self.get_content_by();
        self.tab += 1;
        let access = UnitySeqAccess::new(&mut self, count);
        let ret = visitor.visit_seq(access);
        self.tab -= 1;
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

    fn deserialize_map<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        unimplemented!("deserialize_map")
    }

    fn deserialize_struct<V>(mut self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        //There two type of lines
        //1. TypeName
        //2. (TypeName)
        //3. data (TypeName)
        if self.get_char() == 'd' {
            self.skip(5);
        }
        log::trace!("deserialize_struct:input='{}'", self.peek_line());
        let tab = self.tab;
        self.skip(1);
        let id = self.peek_type()?;
        log::trace!("deserialize_struct: id={}, tab = {}", id, tab+1);
        self.skip_line();
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
        //input='identifier data (type)'
        log::trace!("deserialize_identifier:input='{}'", self.peek_line());
        let id = self.get_string();
        visitor.visit_str(id)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error> where
        V: Visitor<'de> {
        log::trace!("deserialize_ignored_any:input='{}'", self.peek_line());
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
    type Error = UnityDeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<<K as DeserializeSeed<'de>>::Value>, Self::Error> where
        K: DeserializeSeed<'de> {
        let tab = self.de.tab_count();
        //input='\t\tName data (type)'
        log::trace!("next_key_seed:input='{}'", self.de.peek_line());
        if tab < self.tab {
            log::trace!("-----end struct:{}----", self.tab);
            return Ok(None);
        }

        self.de.skip(tab);
        self.de.status.push(DeStatus::StructKey);
        let ret = seed.deserialize(&mut *self.de).map(Some);
        self.de.status.pop();
        ret
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<<V as DeserializeSeed<'de>>::Value, Self::Error> where
        V: DeserializeSeed<'de> {
        //input='data (type)'
        log::trace!("next_value_seed:input='{}'", self.de.peek_line());
        self.de.status.push(DeStatus::StructValue);
        let ret = seed.deserialize(&mut *self.de);
        self.de.status.pop();
        ret
    }
}

struct UnitySeqAccess<'a, 'de: 'a> {
    tab: usize,
    de: &'a mut UnityDeserializer<'de>,
    current: usize,
    count: usize,
    multiple: bool,
}

impl<'a, 'de> UnitySeqAccess<'a, 'de> {
    fn new(de: &'a mut UnityDeserializer<'de>, count: usize) -> Self {
        UnitySeqAccess {
            tab: de.tab,
            current: 0,
            multiple: false,
            de,
            count,
        }
    }
}

const kArrayMemberColumns: usize = 25;

impl<'a, 'de> SeqAccess<'de> for UnitySeqAccess<'a, 'de> {
    type Error = UnityDeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<<T as DeserializeSeed<'de>>::Value>, Self::Error> where
        T: DeserializeSeed<'de> {
        //input='\t\tdata  (type)'
        //input='\t\tdata (type) #0: value value ...'
        //input='\t\tdata (data,data) (type)...'
        if self.current == 0 && self.count != 0 {
            if self.de.is_seq_multi() {
                //TODO Vector3f
                self.multiple = true;
                self.de.status.push(DeStatus::MultipleElement);
            } else {
                self.multiple = false;
                self.de.status.push(DeStatus::SingleElement);
            }
            //log::trace!("next_element_seed:input='{}'", self.de.get_line());
        }
        if self.current == self.count {
            log::trace!("seq end at {}", self.current);
            if self.count != 0 {
                self.de.status.pop();
            }
            self.de.skip_line();
            return Ok(None);
        }

        if self.multiple {
            if self.current % kArrayMemberColumns == 0 {
                self.de.skip_array_header();
            }
        } else {
            self.de.skip(self.tab);
        }
        self.current += 1;
        //log::trace!("next_element_seed:input='{}'", self.de.get_line());
        seed.deserialize(&mut *self.de).map(Some)
    }
}