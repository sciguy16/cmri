pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
pub enum Error {
    OutOfBounds,
    DataTooLong,
    MissingAddress,
    MissingType,
    InvalidMessageType,
}
