#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Custom(&'static str),
    ParseError(&'static str),
}

pub type Result<T> = core::result::Result<T, Error>;
