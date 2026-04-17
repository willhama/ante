use crate::errors::AnteError;
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

/// Maximum file size: 10 MB.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Number of bytes to scan for null bytes (binary detection).
const BINARY_CHECK_LEN: usize = 8192;

#[derive(Serialize)]
pub struct FilePayload {
    pub path: String,
    pub contents: String,
}

#[derive(Serialize)]
pub struct SaveAsResult {
    pub path: String,
}

/// Returns true if the buffer contains null bytes in the first BINARY_CHECK_LEN bytes.
fn is_binary(buf: &[u8]) -> bool {
    let check_len = buf.len().min(BINARY_CHECK_LEN);
    buf[..check_len].contains(&0)
}

/// Validates that a byte buffer is valid UTF-8 and returns the string.
fn validate_utf8(buf: Vec<u8>, path: &str) -> Result<String, AnteError> {
    String::from_utf8(buf).map_err(|_| {
        AnteError::NotUtf8(format!("File is not valid UTF-8: {}", path))
    })
}

/// Opens a native file dialog and reads the selected file.
#[tauri::command]
pub async fn open_file(app: tauri::AppHandle) -> Result<FilePayload, AnteError> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("HTML Files", &["html", "htm"])
        .add_filter("Text Files", &["txt", "md", "text", "markdown", "rst", "log"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    let file_path = match file_path {
        Some(p) => p.to_string(),
        None => return Err(AnteError::DialogCancelled),
    };

    // Check file size before reading
    let metadata = tokio::fs::metadata(&file_path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(AnteError::FileTooLarge(format!(
            "File is {} bytes (limit: {} bytes)",
            metadata.len(),
            MAX_FILE_SIZE
        )));
    }

    let buf = tokio::fs::read(&file_path).await?;

    if is_binary(&buf) {
        return Err(AnteError::BinaryFile(format!(
            "File appears to be binary: {}",
            file_path
        )));
    }

    let contents = validate_utf8(buf, &file_path)?;

    Ok(FilePayload {
        path: file_path,
        contents,
    })
}

/// Saves file contents to the given path using atomic write (temp + rename).
/// Falls back to direct write if rename fails.
#[tauri::command]
pub async fn save_file(path: String, contents: String) -> Result<(), AnteError> {
    atomic_write(&path, &contents).await
}

/// Opens a native save dialog and writes the file to the chosen path.
#[tauri::command]
pub async fn save_file_as(
    app: tauri::AppHandle,
    contents: String,
    suggested_name: Option<String>,
) -> Result<SaveAsResult, AnteError> {
    let mut dialog = app.dialog().file();

    if let Some(name) = suggested_name {
        dialog = dialog.set_file_name(&name);
    }

    let file_path = dialog.blocking_save_file();

    let file_path = match file_path {
        Some(p) => p.to_string(),
        None => return Err(AnteError::DialogCancelled),
    };

    atomic_write(&file_path, &contents).await?;

    Ok(SaveAsResult { path: file_path })
}

/// Writes contents to a file atomically: write to temp file in the same directory,
/// then rename. Falls back to direct overwrite if rename fails.
async fn atomic_write(path: &str, contents: &str) -> Result<(), AnteError> {
    let tmp_path = format!("{}.tmp~", path);

    tokio::fs::write(&tmp_path, contents.as_bytes()).await?;

    match tokio::fs::rename(&tmp_path, path).await {
        Ok(()) => Ok(()),
        Err(_rename_err) => {
            // Fallback: direct overwrite. Remove temp file best-effort.
            let result = tokio::fs::write(path, contents.as_bytes()).await;
            let _ = tokio::fs::remove_file(&tmp_path).await;
            result.map_err(AnteError::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_detection_finds_null_bytes() {
        let buf = b"Hello\x00World";
        assert!(is_binary(buf));
    }

    #[test]
    fn test_binary_detection_clean_utf8() {
        let buf = b"Hello, World! This is plain text.";
        assert!(!is_binary(buf));
    }

    #[test]
    fn test_binary_detection_null_at_boundary() {
        // Null byte right at position 8191 (within check range)
        let mut buf = vec![b'A'; BINARY_CHECK_LEN];
        buf[BINARY_CHECK_LEN - 1] = 0;
        assert!(is_binary(&buf));
    }

    #[test]
    fn test_binary_detection_null_past_boundary() {
        // Null byte at position 8192 (outside check range)
        let mut buf = vec![b'A'; BINARY_CHECK_LEN + 10];
        buf[BINARY_CHECK_LEN] = 0;
        assert!(!is_binary(&buf));
    }

    #[test]
    fn test_utf8_validation_accepts_valid() {
        let buf = "Hello, world!".as_bytes().to_vec();
        let result = validate_utf8(buf, "test.txt");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world!");
    }

    #[test]
    fn test_utf8_validation_rejects_invalid() {
        let buf = vec![0xFF, 0xFE, 0x00, 0x41];
        let result = validate_utf8(buf, "bad.txt");
        assert!(result.is_err());
        match result.unwrap_err() {
            AnteError::NotUtf8(_) => {}
            other => panic!("Expected NotUtf8, got: {:?}", other),
        }
    }

    #[test]
    fn test_utf8_validation_accepts_multibyte() {
        let buf = "Hallo, Welt! Gruesse aus Oesterreich".as_bytes().to_vec();
        let result = validate_utf8(buf, "utf8.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_file_is_valid() {
        let buf: Vec<u8> = vec![];
        assert!(!is_binary(&buf));
        let result = validate_utf8(buf, "empty.txt");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }
}
