use crate::errors::AnteError;
use base64::Engine as _;
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

/// Maximum file size: 10 MB.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Number of bytes to scan for null bytes (binary detection).
const BINARY_CHECK_LEN: usize = 8192;

/// Discriminated payload returned by `open_file` / `read_file` so the
/// frontend can branch on file kind without re-sniffing the path.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpenedFile {
    /// Plain-text file (HTML, MD, TXT, ...). Validated UTF-8.
    Text { path: String, contents: String },
    /// Binary `.docx` file. `bytes_b64` is the file contents base64-encoded.
    Docx { path: String, bytes_b64: String },
}

#[derive(Serialize)]
pub struct SaveAsResult {
    pub path: String,
}

#[derive(Serialize)]
pub struct DirEntryInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Serialize)]
pub struct PickedImage {
    pub src: String,
    pub title: String,
}

/// Maximum number of entries returned from list_directory. Prevents pathological
/// folder reads (node_modules, etc.) from flooding the renderer.
const MAX_DIR_ENTRIES: usize = 500;

/// List the immediate children of a directory. If `path` is a file, lists its
/// parent directory instead. Skips dotfiles. Sorted: directories first, then
/// files, alphabetical within each group. Capped at MAX_DIR_ENTRIES.
#[tauri::command]
pub async fn list_directory(path: String) -> Result<Vec<DirEntryInfo>, AnteError> {
    let p = std::path::Path::new(&path);
    let dir = if p.is_file() {
        p.parent()
            .ok_or_else(|| AnteError::Io("path has no parent".to_string()))?
            .to_path_buf()
    } else {
        p.to_path_buf()
    };

    let mut entries: Vec<DirEntryInfo> = Vec::new();
    let mut read = tokio::fs::read_dir(&dir).await?;
    while let Some(entry) = read.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let metadata = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };
        entries.push(DirEntryInfo {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
        });
        if entries.len() >= MAX_DIR_ENTRIES {
            break;
        }
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

/// Create a new empty `.html` document in the given directory and return the
/// chosen path. If "Untitled.html" already exists, increments the suffix
/// ("Untitled 2.html", "Untitled 3.html", ...) until a free name is found.
/// Writes a minimal empty body so the file appears immediately in directory
/// listings.
#[tauri::command]
pub async fn create_document(dir: String) -> Result<SaveAsResult, AnteError> {
    let dir_path = std::path::Path::new(&dir);
    if !dir_path.is_dir() {
        return Err(AnteError::Io(format!("not a directory: {}", dir)));
    }
    let mut chosen: Option<std::path::PathBuf> = None;
    for n in 0..1000 {
        let name = if n == 0 {
            "Untitled.html".to_string()
        } else {
            format!("Untitled {}.html", n + 1)
        };
        let candidate = dir_path.join(&name);
        if !candidate.exists() {
            chosen = Some(candidate);
            break;
        }
    }
    let path = chosen.ok_or_else(|| {
        AnteError::Io("could not find a free Untitled name".to_string())
    })?;
    tokio::fs::write(&path, b"").await?;
    Ok(SaveAsResult {
        path: path.to_string_lossy().to_string(),
    })
}

/// Move a file into a destination directory. Preserves the source filename.
/// Refuses to overwrite an existing target. Tries `rename` first and falls
/// back to copy + remove if the rename hits a cross-device boundary.
#[tauri::command]
pub async fn move_path(src: String, dst_dir: String) -> Result<SaveAsResult, AnteError> {
    let src_path = std::path::Path::new(&src);
    let dst_dir_path = std::path::Path::new(&dst_dir);
    if !src_path.exists() {
        return Err(AnteError::Io(format!("source does not exist: {}", src)));
    }
    if !dst_dir_path.is_dir() {
        return Err(AnteError::Io(format!(
            "destination is not a directory: {}",
            dst_dir
        )));
    }
    let name = src_path
        .file_name()
        .ok_or_else(|| AnteError::Io("source has no filename".to_string()))?;
    let new_path = dst_dir_path.join(name);
    if new_path == src_path {
        return Ok(SaveAsResult {
            path: new_path.to_string_lossy().to_string(),
        });
    }
    if new_path.exists() {
        return Err(AnteError::Io(format!(
            "target already exists: {}",
            new_path.display()
        )));
    }
    if tokio::fs::rename(src_path, &new_path).await.is_err() {
        tokio::fs::copy(src_path, &new_path).await?;
        tokio::fs::remove_file(src_path).await?;
    }
    Ok(SaveAsResult {
        path: new_path.to_string_lossy().to_string(),
    })
}

/// Reads a file at the given path. Used by the sidebar to open a file the user
/// clicked, bypassing the native picker dialog. Routes `.docx` to the binary
/// path; everything else is read as UTF-8 text with the usual guards.
#[tauri::command]
pub async fn read_file(path: String) -> Result<OpenedFile, AnteError> {
    let metadata = tokio::fs::metadata(&path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(AnteError::FileTooLarge(format!(
            "File is {} bytes (limit: {} bytes)",
            metadata.len(),
            MAX_FILE_SIZE
        )));
    }
    let buf = tokio::fs::read(&path).await?;

    if is_docx_path(&path) {
        let bytes_b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        return Ok(OpenedFile::Docx { path, bytes_b64 });
    }

    if is_binary(&buf) {
        return Err(AnteError::BinaryFile(format!(
            "File appears to be binary: {}",
            path
        )));
    }
    let contents = validate_utf8(buf, &path)?;
    Ok(OpenedFile::Text { path, contents })
}

/// Returns true if the path ends with `.docx` (case-insensitive).
fn is_docx_path(path: &str) -> bool {
    path.to_lowercase().ends_with(".docx")
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

/// Opens a native file picker filtered to images, reads the selected file,
/// and returns a base64-encoded data URI alongside the source filename.
#[tauri::command]
pub async fn pick_image(app: tauri::AppHandle) -> Result<PickedImage, AnteError> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "svg"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    let path = match file_path {
        Some(p) => p.to_string(),
        None => return Err(AnteError::DialogCancelled),
    };

    let metadata = tokio::fs::metadata(&path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(AnteError::FileTooLarge(format!(
            "Image is {} bytes (limit: {} bytes)",
            metadata.len(),
            MAX_FILE_SIZE
        )));
    }

    let bytes = tokio::fs::read(&path).await?;
    let mime = mime_from_path(&path);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let title = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string();
    Ok(PickedImage {
        src: format!("data:{};base64,{}", mime, encoded),
        title,
    })
}

fn mime_from_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "image/octet-stream"
    }
}

/// Opens a native file dialog and reads the selected file. Routes `.docx`
/// through the binary path; everything else is read as UTF-8 text.
#[tauri::command]
pub async fn open_file(app: tauri::AppHandle) -> Result<OpenedFile, AnteError> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("All Supported", &["html", "htm", "docx", "txt", "md", "text", "markdown", "rst", "log"])
        .add_filter("HTML Files", &["html", "htm"])
        .add_filter("Word Documents", &["docx"])
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

    if is_docx_path(&file_path) {
        let bytes_b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        return Ok(OpenedFile::Docx {
            path: file_path,
            bytes_b64,
        });
    }

    if is_binary(&buf) {
        return Err(AnteError::BinaryFile(format!(
            "File appears to be binary: {}",
            file_path
        )));
    }

    let contents = validate_utf8(buf, &file_path)?;

    Ok(OpenedFile::Text {
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

/// Opens a native save dialog filtered to `.docx` and writes binary bytes
/// (decoded from base64) to the chosen path. Used for Word document export.
#[tauri::command]
pub async fn save_docx_as(
    app: tauri::AppHandle,
    bytes_b64: String,
    suggested_name: Option<String>,
) -> Result<SaveAsResult, AnteError> {
    let mut dialog = app
        .dialog()
        .file()
        .add_filter("Word Document", &["docx"]);

    if let Some(name) = suggested_name {
        dialog = dialog.set_file_name(&name);
    }

    let file_path = match dialog.blocking_save_file() {
        Some(p) => p.to_string(),
        None => return Err(AnteError::DialogCancelled),
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(bytes_b64.as_bytes())
        .map_err(|e| AnteError::Io(format!("invalid base64 payload: {}", e)))?;

    atomic_write_bytes(&file_path, &bytes).await?;

    Ok(SaveAsResult { path: file_path })
}

/// Writes contents to a file atomically: write to temp file in the same directory,
/// then rename. Falls back to direct overwrite if rename fails.
async fn atomic_write(path: &str, contents: &str) -> Result<(), AnteError> {
    atomic_write_bytes(path, contents.as_bytes()).await
}

/// Binary-safe version of `atomic_write`. Used for `.docx` export.
async fn atomic_write_bytes(path: &str, bytes: &[u8]) -> Result<(), AnteError> {
    let tmp_path = format!("{}.tmp~", path);

    tokio::fs::write(&tmp_path, bytes).await?;

    match tokio::fs::rename(&tmp_path, path).await {
        Ok(()) => Ok(()),
        Err(_rename_err) => {
            let result = tokio::fs::write(path, bytes).await;
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
    fn test_mime_from_path_known_extensions() {
        assert_eq!(mime_from_path("photo.jpg"), "image/jpeg");
        assert_eq!(mime_from_path("photo.jpeg"), "image/jpeg");
        assert_eq!(mime_from_path("PHOTO.JPEG"), "image/jpeg");
        assert_eq!(mime_from_path("scan.PNG"), "image/png");
        assert_eq!(mime_from_path("anim.gif"), "image/gif");
        assert_eq!(mime_from_path("shot.webp"), "image/webp");
        assert_eq!(mime_from_path("icon.svg"), "image/svg+xml");
    }

    #[test]
    fn test_mime_from_path_unknown_and_missing() {
        assert_eq!(mime_from_path("data.bin"), "image/octet-stream");
        assert_eq!(mime_from_path("no-extension"), "image/octet-stream");
        assert_eq!(mime_from_path(""), "image/octet-stream");
    }

    #[test]
    fn test_empty_file_is_valid() {
        let buf: Vec<u8> = vec![];
        assert!(!is_binary(&buf));
        let result = validate_utf8(buf, "empty.txt");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_is_docx_path_recognizes_extension() {
        assert!(is_docx_path("document.docx"));
        assert!(is_docx_path("/path/to/Report.DOCX"));
        assert!(is_docx_path("My File.Docx"));
    }

    #[test]
    fn test_is_docx_path_rejects_non_docx() {
        assert!(!is_docx_path("document.html"));
        assert!(!is_docx_path("document.doc"));
        assert!(!is_docx_path("docx"));
        assert!(!is_docx_path("docx.html"));
        assert!(!is_docx_path(""));
    }

    #[test]
    fn test_opened_file_text_serialization_tag() {
        let payload = OpenedFile::Text {
            path: "/tmp/x.html".into(),
            contents: "<p>hi</p>".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"kind\":\"text\""));
        assert!(json.contains("\"contents\":\"<p>hi</p>\""));
    }

    #[test]
    fn test_opened_file_docx_serialization_tag() {
        let payload = OpenedFile::Docx {
            path: "/tmp/x.docx".into(),
            bytes_b64: "UEsDBBQAAAA".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"kind\":\"docx\""));
        assert!(json.contains("\"bytes_b64\":\"UEsDBBQAAAA\""));
    }
}
