#[cfg(feature = "std")]
use std::format;
#[cfg(feature = "std")]
use std::string::String;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
pub enum Error {
    OutOfBounds,
    DataTooLong,
    MissingAddress,
    MissingType,
    InvalidMessageType,
    InvalidNodeType,
    #[cfg(feature = "std")]
    IoError(String),
}

impl core::fmt::Display for Error {
    fn fmt(
        &self,
        fmt: &mut core::fmt::Formatter<'_>,
    ) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{:?}", self)
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(format!("{}", e))
    }
}
