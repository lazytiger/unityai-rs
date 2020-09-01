mod deserializer;
pub use deserializer::UnityDeserializer;
pub use deserializer::from_str;

use serde::export::fmt::Display;
use serde::export::Formatter;
use std::num::ParseIntError;

#[derive(Debug)]
pub struct Error {

}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self where
        T: Display {
        unimplemented!()
    }
}

impl std::error::Error for Error {

}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unimplemented!()
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::convert::From<std::num::ParseIntError> for Error {
    fn from(err: ParseIntError) -> Self {
        unimplemented!()
    }
}