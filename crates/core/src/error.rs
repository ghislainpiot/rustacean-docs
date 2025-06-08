use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("General error: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, Error>;