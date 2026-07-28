use thiserror::Error;

#[derive(Debug, Error)]
pub enum PetError {
    #[error("floating pet is only supported on Android")]
    UnsupportedPlatform,

    #[error("overlay permission denied")]
    PermissionDenied,

    #[error("floating pet service is not running")]
    ServiceNotRunning,

    #[error("android jni error: {0}")]
    Jni(String),

    #[error("invalid state payload: {0}")]
    InvalidPayload(String),

    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for PetError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
