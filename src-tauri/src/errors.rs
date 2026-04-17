use serde::Serialize;

/// Structured error type for all ante Tauri commands.
/// Serialized as JSON over IPC so the frontend can branch on `kind`.
#[derive(Debug, thiserror::Error)]
pub enum AnteError {
    #[error("io: {0}")]
    Io(String),

    #[error("not_utf8: {0}")]
    NotUtf8(String),

    #[error("binary_file: {0}")]
    BinaryFile(String),

    #[error("dialog_cancelled")]
    DialogCancelled,

    #[error("file_too_large: {0}")]
    FileTooLarge(String),

    #[error("api_error: {0}")]
    ApiError(String),
}

/// Serialization format sent to the frontend over Tauri IPC.
#[derive(Serialize)]
struct AnteErrorPayload {
    kind: &'static str,
    message: String,
}

impl Serialize for AnteError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let payload = match self {
            AnteError::Io(msg) => AnteErrorPayload {
                kind: "io",
                message: msg.clone(),
            },
            AnteError::NotUtf8(msg) => AnteErrorPayload {
                kind: "not_utf8",
                message: msg.clone(),
            },
            AnteError::BinaryFile(msg) => AnteErrorPayload {
                kind: "binary_file",
                message: msg.clone(),
            },
            AnteError::DialogCancelled => AnteErrorPayload {
                kind: "dialog_cancelled",
                message: String::new(),
            },
            AnteError::FileTooLarge(msg) => AnteErrorPayload {
                kind: "file_too_large",
                message: msg.clone(),
            },
            AnteError::ApiError(msg) => AnteErrorPayload {
                kind: "api_error",
                message: msg.clone(),
            },
        };
        payload.serialize(serializer)
    }
}

impl From<std::io::Error> for AnteError {
    fn from(err: std::io::Error) -> Self {
        AnteError::Io(err.to_string())
    }
}
