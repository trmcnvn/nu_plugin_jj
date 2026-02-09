use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("jj: {0}")]
    Jj(String),
}
