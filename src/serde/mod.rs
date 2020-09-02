use std::num::{ParseFloatError, ParseIntError};

use serde::de::{Error, Unexpected, Visitor};
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

struct Vector3fVistor;

impl<'de> Visitor<'de> for Vector3fVistor {
    type Value = Vector3f;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("(f32, f32, f32)")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: Error,
    {
        let err = Error::invalid_value(Unexpected::Other(v), &self);
        let bgn = v.chars().position(|c| c == '(').ok_or_else(|| err)?;

        let err = Error::invalid_value(Unexpected::Other(&v[bgn..]), &self);
        let end = v[bgn + 1..]
            .chars()
            .position(|c| c == ')')
            .ok_or_else(|| err)?;

        let mut content = v[bgn + 1..bgn + end + 1].split_ascii_whitespace();

        let err = Error::invalid_value(Unexpected::Other(v), &self);
        let err2 = Error::invalid_value(Unexpected::Other(v), &self);
        let x = content
            .next()
            .ok_or_else(|| err)?
            .parse()
            .or_else(|_| Err(err2))?;

        let err = Error::invalid_value(Unexpected::Other(v), &self);
        let err2 = Error::invalid_value(Unexpected::Other(v), &self);
        let y = content
            .next()
            .ok_or_else(|| err)?
            .parse()
            .or_else(|_| Err(err2))?;

        let err = Error::invalid_value(Unexpected::Other(v), &self);
        let err2 = Error::invalid_value(Unexpected::Other(v), &self);
        let z = content
            .next()
            .ok_or_else(|| err)?
            .parse()
            .or_else(|_| Err(err2))?;

        log::trace!("vector3f {} {} {}", x, y, z);
        Ok(Vector3f { x, y, z })
    }
}

impl<'de> Deserialize<'de> for Vector3f {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Vector3fVistor)
    }
}
