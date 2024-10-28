#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("custom error {0}")]
    Custom(&'static str),
    #[error("parse error {0}")]
    ParseError(&'static str),
}

pub type Result<T> = core::result::Result<T, Error>;
