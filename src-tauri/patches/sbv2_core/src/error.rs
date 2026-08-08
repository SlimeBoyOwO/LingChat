use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Tokenizer error: {0}")]
    TokenizerError(#[from] tokenizers::Error),
    #[error("JPreprocess error: {0}")]
    JPreprocessError(#[from] jpreprocess::error::JPreprocessError),
    #[error("Lindera error: {0}")]
    LinderaError(String),
    #[cfg(feature = "std")]
    #[error("ONNX error: {0}")]
    OrtError(#[from] ort::Error),
    #[error("NDArray error: {0}")]
    NdArrayError(#[from] ndarray::ShapeError),
    #[error("Value error: {0}")]
    ValueError(String),
    #[error("Serde_json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("hound error: {0}")]
    HoundError(#[from] hound::Error),
    #[error("model not found error")]
    ModelNotFoundError(String),
    #[cfg(feature = "base64")]
    #[error("base64 error")]
    Base64Error(#[from] base64::DecodeError),
    #[error("other")]
    OtherError(String),
    #[error("Style error: {0}")]
    StyleError(String),
}

#[cfg(feature = "std")]
impl From<ort::error::Error<ort::session::builder::SessionBuilder>> for Error {
    fn from(err: ort::error::Error<ort::session::builder::SessionBuilder>) -> Self {
        Error::OrtError(err.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
