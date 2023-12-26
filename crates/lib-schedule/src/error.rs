#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Custom(&'static str),
    ParseError,
}

pub type Result<T> = core::result::Result<T, Error>;
