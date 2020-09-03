use std::str::{Chars, FromStr};

use regex::Regex;
use serde::de::{DeserializeSeed, Error, Expected, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

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
    root: bool,
    type_name: String,
}

impl<'de> UnityDeserializer<'de> {
    fn from_str(data: &'de str) -> UnityDeserializer<'de> {
        let mut status = Vec::new();
        status.push(DeStatus::Invalid);
        let regex = Regex::new(r"data \([0-9a-zA-Z ]+\) #[0-9]+:").unwrap();
        UnityDeserializer {
            data,
            tab: 0,
            offset: 0,
            root: true,
            status,
            regex,
            type_name: String::new(),
        }
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

    fn skip_header(&mut self) -> super::Result<()> {
        let mut current_eol = 0;
        let pos = self
            .chars()
            .position(|d| {
                if d == '\n' {
                    current_eol += 1;
                } else if d != '\r' {
                    current_eol = 0;
                }
                current_eol == 3
            })
            .ok_or_else(|| UnityDeError::custom("skip file header failed"))?;
        self.skip(pos + 1)
    }

    fn count_until(&self, d: char) -> usize {
        self.chars()
            .position(|c| c == d)
            .unwrap_or_else(|| self.remaining())
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.offset
    }

    fn chars(&self) -> Chars {
        self.data[self.offset..].chars()
    }

    fn skip(&mut self, count: usize) -> super::Result<()> {
        if self.offset + count <= self.data.len() {
            self.offset += count;
            Ok(())
        } else {
            Err(UnityDeError::Eof)
        }
    }

    fn skip_tab(&mut self, count: usize) -> super::Result<()> {
        let mut it = self.chars();
        for _ in 0..count {
            if it.next().ok_or_else(|| UnityDeError::Eof)? != '\t' {
                return Err(UnityDeError::custom(format!(
                    "tab not match:{}",
                    self.peek_line()
                )));
            }
        }
        self.skip(count)
    }

    fn skip_space(&mut self) -> super::Result<()> {
        if !self.next_char()?.is_ascii_whitespace() {
            Err(UnityDeError::custom(format!(
                "space expected at:{}",
                self.peek_line()
            )))
        } else {
            Ok(())
        }
    }

    fn skip_until(&mut self, d: char) -> super::Result<()> {
        let pos = self.count_until(d);
        self.skip(pos + 1)
    }

    fn skip_line(&mut self) -> super::Result<()> {
        if let DeStatus::MultipleElement = self.current_status() {
            Ok(())
        } else {
            self.skip_until('\n')
        }
    }

    fn get_str(&mut self, len: usize) -> super::Result<&'de str> {
        if self.offset + len > self.data.len() {
            Err(UnityDeError::Eof)
        } else {
            let ret = &self.data[self.offset..self.offset + len];
            self.skip(len)?;
            Ok(ret)
        }
    }

    fn peek_str(&self, len: usize) -> super::Result<&'de str> {
        if self.offset + len <= self.data.len() {
            Ok(&self.data[self.offset..self.offset + len])
        } else {
            Err(UnityDeError::Eof)
        }
    }

    fn peek_type(&mut self) -> super::Result<&str> {
        let line = self.peek_line();
        let (bgn, _) = line
            .char_indices()
            .rfind(|(_, c)| *c == '(')
            .ok_or_else(|| UnityDeError::custom(format!("type not found:{}", line)))?;
        let end = line[bgn + 1..]
            .chars()
            .position(|c| c == ')')
            .ok_or_else(|| UnityDeError::custom(format!("type not found:{}", line)))?;
        Ok(&line[bgn + 1..bgn + end + 1])
    }

    fn get_identifier(&mut self) -> super::Result<&str> {
        let pos = self
            .chars()
            .position(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '[' && c != ']')
            .ok_or_else(|| UnityDeError::custom("identifier not found"))?;
        self.get_str(pos)
    }

    fn next_char(&mut self) -> super::Result<char> {
        let ret = self.chars().next().ok_or_else(|| UnityDeError::Eof)?;
        self.skip(1)?;
        Ok(ret)
    }

    fn get_content(&mut self) -> super::Result<&str> {
        let pos = self
            .chars()
            .position(|c| c == ' ' || c == '\r' || c == '\n')
            .ok_or_else(|| UnityDeError::Eof)?;
        self.get_str(pos)
    }

    fn get_content_by<T: FromStr>(&mut self) -> super::Result<T> {
        let content = self.get_content()?;
        match T::from_str(content) {
            Ok(t) => {
                self.skip_line()?;
                Ok(t)
            }
            Err(_) => Err(UnityDeError::custom(format!("parse '{}' failed", content))),
        }
    }

    fn skip_array_header(&mut self) -> super::Result<()> {
        let count = self.count_until(':');
        self.skip(count + 1)
    }

    fn peek_line(&self) -> &str {
        let pos = self
            .chars()
            .position(|c| c == '\r' || c == '\n')
            .unwrap_or(self.data.len() - self.offset);
        self.peek_str(pos).unwrap_or("")
    }

    fn is_seq_multi(&self) -> super::Result<bool> {
        Ok(self.regex.is_match(self.peek_line()))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.data.len()
    }
}

pub fn from_str<'a, T: Deserialize<'a>>(data: &'a str) -> super::Result<T> {
    let mut de = UnityDeserializer::from_str(data);
    de.skip_header()?;
    de.skip_until(')')?;
    let t = T::deserialize(&mut de)?;
    de.skip_line()?;
    de.skip_line()?;
    if de.is_empty() {
        Ok(t)
    } else {
        Err(UnityDeError::custom(format!(
            "tailing data:'{}'",
            de.peek_line()
        )))
    }
}

impl<'de, 'a> Deserializer<'de> for &'a mut UnityDeserializer<'de> {
    type Error = UnityDeError;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.current_status() {
            DeStatus::StructKey => {
                //name content type
                log::trace!("deserialize_any:StructKey, input='{}'", self.peek_line());
                self.deserialize_identifier(visitor)
            }
            DeStatus::Invalid => unreachable!("invalid status"),
            _ => {
                //2. content type
                self.type_name = if let DeStatus::MultipleElement = self.current_status() {
                    self.type_name.as_str()
                } else {
                    self.peek_type()?
                }
                .into();
                log::trace!(
                    "deserialize_any:StructValue, type={}, input='{}'",
                    self.type_name,
                    self.peek_line()
                );
                match self.type_name.as_str() {
                    "vector" => self.deserialize_seq(visitor),
                    "SInt64" => self.deserialize_i64(visitor),
                    "unsigned int" => self.deserialize_u32(visitor),
                    "int" => self.deserialize_i32(visitor),
                    "string" => self.deserialize_string(visitor),
                    "UInt8" | "unsigned char" => self.deserialize_u8(visitor),
                    "float" => self.deserialize_f32(visitor),
                    "Vector3f" => self.deserialize_str(visitor),
                    "unsigned short|UInt16" => self.deserialize_u16(visitor),
                    _ => self.deserialize_struct("", &[], visitor),
                }
            }
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.get_content_by()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.get_content_by()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.get_content_by()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.get_content_by()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.get_content_by()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.get_content_by()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.get_content_by()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.get_content_by()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.get_content_by()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(self.get_content_by()?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(self.get_content_by()?)
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_char")
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let id = self.peek_line();
        let ret = visitor.visit_str(id);
        self.skip_line()?;
        ret
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let content = self.get_content()?;
        let len = content.len();
        let content = content[1..len - 1].into();
        self.skip_line()?;
        visitor.visit_string(content)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_bytes")
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_byte_buf")
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_option")
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_unit")
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_unit_struct")
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_newtype_struct")
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        //begin as ' (vector)'
        log::trace!("deserialize_seq:input='{}'", self.peek_line());
        self.skip_line()?;

        let typ = format!("{}", &visitor as &dyn Expected);
        let (count, faked) = if typ.as_str() == "Hash128" {
            (16, true)
        } else {
            //current:\t+ size xxx (int)
            log::trace!("deserialize_seq:input='{}'", self.peek_line());
            self.skip_tab(self.tab_count())?;
            if self.get_identifier()? != "size" {
                return Err(UnityDeError::custom("no size found"));
            }
            // 57 (int)
            log::trace!("deserialize_seq:input='{}'", self.peek_line());
            self.skip_space()?;
            (self.get_content_by()?, false)
        };

        self.tab += 1;
        let access = UnitySeqAccess::new(&mut self, count, faked);
        let ret = visitor.visit_seq(access);
        self.tab -= 1;
        ret
    }

    fn deserialize_tuple<V>(
        self,
        _len: usize,
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_tuple")
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_tuple_struct")
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_map")
    }

    fn deserialize_struct<V>(
        mut self,
        name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        //There two type of lines
        //1. TypeName
        //2. (TypeName)
        //3. data (TypeName)
        log::trace!("deserialize_struct:input='{}'", self.peek_line());
        let tab = self.tab;
        self.skip_space()?;
        let id = if self.root {
            self.root = false;
            self.get_identifier()?
        } else {
            self.peek_type()?
        };
        if name != "" && name != id {
            return Err(UnityDeError::custom(format!(
                "type {} not match {}",
                name, id
            )));
        }
        log::trace!("deserialize_struct: id={}, tab = {}", id, tab + 1);
        self.skip_line()?;
        self.tab += 1;
        let access = UnityMapAccess::new(&mut self);
        let ret = visitor.visit_map(access);
        self.tab -= 1;
        ret
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("deserialize_enum")
    }

    fn deserialize_identifier<V>(
        self,
        visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        //input='identifier data (type)'
        log::trace!("deserialize_identifier:input='{}'", self.peek_line());
        let id = self.get_identifier()?;
        visitor.visit_str(id)
    }

    fn deserialize_ignored_any<V>(
        self,
        visitor: V,
    ) -> Result<<V as Visitor<'de>>::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
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

    fn next_key_seed<K>(
        &mut self,
        seed: K,
    ) -> Result<Option<<K as DeserializeSeed<'de>>::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        let tab = self.de.tab_count();
        //input='\t\tName data (type)'
        log::trace!("next_key_seed:input='{}'", self.de.peek_line());
        if tab < self.tab {
            log::trace!("-----end struct:{}----", self.tab);
            return Ok(None);
        }

        self.de.skip_tab(tab)?;
        self.de.status.push(DeStatus::StructKey);
        let ret = seed.deserialize(&mut *self.de).map(Some);
        self.de.status.pop();
        ret
    }

    fn next_value_seed<V>(
        &mut self,
        seed: V,
    ) -> Result<<V as DeserializeSeed<'de>>::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        //input=' data (type)'
        if self.de.next_char()? != ' ' {
            return Err(UnityDeError::custom(format!(
                "invalid line:{}",
                self.de.peek_line()
            )));
        }
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
    faked: bool,
}

impl<'a, 'de> UnitySeqAccess<'a, 'de> {
    fn new(de: &'a mut UnityDeserializer<'de>, count: usize, faked: bool) -> Self {
        UnitySeqAccess {
            tab: de.tab,
            current: 0,
            multiple: false,
            de,
            count,
            faked,
        }
    }
}

#[allow(non_upper_case_globals)]
const ArrayMemberColumns: usize = 25;

impl<'a, 'de> SeqAccess<'de> for UnitySeqAccess<'a, 'de> {
    type Error = UnityDeError;

    fn next_element_seed<T>(
        &mut self,
        seed: T,
    ) -> Result<Option<<T as DeserializeSeed<'de>>::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        //input='\t\tdata  (type)'
        //input='\t\tdata (type) #0: value value ...'
        //input='\t\tdata (data,data) (type)...'
        if self.current == 0 && self.count != 0 {
            if self.de.is_seq_multi()? {
                self.de.type_name = self.de.peek_type()?.into();
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
            self.de.skip_line()?;
            return Ok(None);
        }

        if self.multiple {
            if self.current % ArrayMemberColumns == 0 {
                self.de.skip_array_header()?;
            }
            self.de.skip_space()?;
        } else {
            self.de.skip_tab(self.tab)?;
            if self.de.get_identifier()? != "data" && !self.faked {
                return Err(UnityDeError::custom(
                    format! {"no data keyword found in seq:{}", self.de.peek_line()},
                ));
            }
            self.de.skip_space()?;
        }
        self.current += 1;
        log::trace!("next_element_seed:input='{}'", self.de.peek_line());
        seed.deserialize(&mut *self.de).map(Some)
    }
}
