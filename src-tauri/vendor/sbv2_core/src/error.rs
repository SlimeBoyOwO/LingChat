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
    OrtError(String),
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

// ort rc.12 made `ort::Error` generic over the operation context
// (SessionBuilder, Session, TensorRef, Tensor, ...). Upstream only declares
// `#[from] ort::Error` which no longer compiles for any specific T, so add a
// blanket From impl here. This file lives in the vendored copy under
// src-tauri/vendor/sbv2_core/; the upstream repo at
// C:\Users\DCY45\Desktop\sbv2-api is not modified.
#[cfg(feature = "std")]
impl<T> From<ort::Error<T>> for Error
where
    ort::Error<T>: std::fmt::Display,
{
    fn from(value: ort::Error<T>) -> Self {
        Error::OrtError(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
