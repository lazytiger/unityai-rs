use serde::de::{Error, SeqAccess, Visitor};
use serde::export::fmt::Display;
use serde::export::Formatter;
use serde::{Deserialize, Deserializer};

pub use deserializer::from_str;
pub use deserializer::UnityDeserializer;

mod deserializer;

#[derive(Debug)]
pub enum UnityDeError {
    Other(String),
    Eof,
}

impl Error for UnityDeError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        let msg = format!("{}", msg);
        UnityDeError::Other(msg)
    }
}

impl std::error::Error for UnityDeError {}

impl std::fmt::Display for UnityDeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UnityDeError::Other(msg) => f.write_str(msg),
            UnityDeError::Eof => f.write_str("end of file"),
        }
    }
}

pub type Result<T> = std::result::Result<T, UnityDeError>;

#[derive(Debug)]
pub struct Vector3f {
    x: f32,
    y: f32,
    z: f32,
}

struct Vector3fVisitor;

impl<'de> Visitor<'de> for Vector3fVisitor {
    type Value = Vector3f;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("(f32, f32, f32)")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: Error,
    {
        let bgn = v
            .chars()
            .position(|c| c == '(')
            .ok_or_else(|| Error::custom(format!("no data found in {}", v)))?;

        let end = v[bgn + 1..]
            .chars()
            .position(|c| c == ')')
            .ok_or_else(|| Error::custom(format!("no data found in {}", &v[bgn + 1..])))?;

        let mut content = v[bgn + 1..bgn + end + 1].split_ascii_whitespace();

        let mut ret = Vector3f {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        for i in 0..3 {
            let d = content
                .next()
                .ok_or_else(|| Error::custom(format!("no data{} found in {}", i, v)))?
                .parse()
                .map_err(|e| Error::custom(format!("{}", e)))?;
            match i {
                0 => ret.x = d,
                1 => ret.y = d,
                2 => ret.z = d,
                _ => unreachable!(),
            }
        }

        Ok(ret)
    }
}

impl<'de> Deserialize<'de> for Vector3f {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Vector3fVisitor)
    }
}

#[derive(Debug)]
pub struct Hash128 {
    bytes: [u8; 16],
}

struct Hash128Visitor;

impl<'de> Visitor<'de> for Hash128Visitor {
    type Value = Hash128;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("Hash128")
    }

    fn visit_seq<A>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, <A as SeqAccess<'de>>::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut hash = Hash128 { bytes: [0u8; 16] };
        for i in 0..16 {
            hash.bytes[i] = seq
                .next_element()?
                .ok_or_else(|| serde::de::Error::custom(format!("Hash128 missing {}th byte", i)))?;
        }
        Ok(hash)
    }
}

impl<'de> Deserialize<'de> for Hash128 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(Hash128Visitor)
    }
}
