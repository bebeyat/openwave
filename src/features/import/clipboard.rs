//! Reading text from the system clipboard (Rust side, so no webview permission prompt).
//! The text stays on this computer; the dialog only looks at it for a playlist link.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardError {
    /// There is no text on the clipboard.
    Empty,
    /// The clipboard could not be opened (another program holds it, or no access).
    Unavailable,
}

fn read_blocking() -> Result<String, ClipboardError> {
    let mut board = arboard::Clipboard::new().map_err(|_| ClipboardError::Unavailable)?;
    match board.get_text() {
        Ok(text) if text.trim().is_empty() => Err(ClipboardError::Empty),
        Ok(text) => Ok(text),
        Err(arboard::Error::ContentNotAvailable) => Err(ClipboardError::Empty),
        Err(_) => Err(ClipboardError::Unavailable),
    }
}

/// The clipboard text (capped to a sane length). Never panics; failures are values.
pub async fn read_text() -> Result<String, ClipboardError> {
    let text = tokio::task::spawn_blocking(read_blocking)
        .await
        .map_err(|_| ClipboardError::Unavailable)??;
    Ok(text.chars().take(4096).collect())
}
