//! Local music files: picker, drag-and-drop and registration.
use super::{backend_ref, run};
use crate::backend::{
    error::{ApiError, ApiResult, MediaError},
    local_tags,
    media::LOCAL_EXTENSIONS,
};
use crate::model::{Lyrics, Track};
use std::path::PathBuf;

fn is_audio_file(path: &std::path::Path) -> bool {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    LOCAL_EXTENSIONS.contains(&ext.as_str()) && path.is_file()
}

/// Largest `.lrc` file read next to a local track.
const MAX_LRC_BYTES: u64 = 1024 * 1024;

/// Register audio files by path (used by the picker, folder scans and drag-and-drop) and read
/// their tags, duration and cover. Non-audio paths are skipped.
pub async fn add_local_files(paths: Vec<PathBuf>) -> ApiResult<Vec<Track>> {
    let b = backend_ref()?;
    run(async move {
        let paths: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .take(local_tags::MAX_SCAN_FILES)
                .filter(|p| is_audio_file(p))
                .collect()
        })
        .await
        .map_err(|e| MediaError::Io(e.to_string()))?;
        let ids = b.media.register_local_batch(paths.clone()).await?;
        let covers = b.media.covers_dir();
        let tracks = tokio::task::spawn_blocking(move || {
            paths
                .iter()
                .zip(ids)
                .map(|(path, id)| {
                    local_tags::to_track(&id, path, local_tags::read_meta(path, &covers))
                })
                .collect::<Vec<Track>>()
        })
        .await
        .map_err(|e| MediaError::Io(e.to_string()))?;
        Ok::<_, MediaError>(tracks)
    })
    .await?
    .map_err(ApiError::from)
}

/// Read the tags of already added local tracks again (files added before tags were read, or
/// edited since). Tracks whose file is gone are left out.
pub async fn refresh_local_tracks(tracks: Vec<Track>) -> ApiResult<Vec<Track>> {
    let b = backend_ref()?;
    run(async move {
        let covers = b.media.covers_dir();
        let known: Vec<(String, PathBuf)> = tracks
            .iter()
            .filter_map(|t| b.media.local_path(&t.id).map(|p| (t.id.to_string(), p)))
            .collect();
        tokio::task::spawn_blocking(move || {
            known
                .into_iter()
                .filter(|(_, p)| p.is_file())
                .map(|(id, p)| local_tags::to_track(&id, &p, local_tags::read_meta(&p, &covers)))
                .collect::<Vec<Track>>()
        })
        .await
        .map_err(|e| MediaError::Io(e.to_string()))
    })
    .await?
    .map_err(ApiError::from)
}

/// Lyrics from the `.lrc` file next to a local track (read from disk only, nothing is sent).
pub async fn local_lyrics(track_id: &str) -> ApiResult<Option<Lyrics>> {
    let b = backend_ref()?;
    let Some(path) = b.media.local_path(track_id) else {
        return Ok(None);
    };
    let Ok(Some(lrc)) = tokio::task::spawn_blocking(move || local_tags::sidecar_lrc(&path)).await
    else {
        return Ok(None);
    };
    // A lyrics file is a few KiB; anything huge is not one.
    if tokio::fs::metadata(&lrc)
        .await
        .map_or(true, |m| m.len() > MAX_LRC_BYTES)
    {
        return Ok(None);
    }
    let text = tokio::fs::read(&lrc)
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|e| ApiError::from(MediaError::from(e)))?;
    let mut lyrics = crate::backend::lyrics::parse(&text);
    lyrics.provider = "LRC".into();
    Ok((!lyrics.is_empty()).then_some(lyrics))
}

/// Largest picture accepted as a playlist cover.
const MAX_COVER_BYTES: u64 = 16 * 1024 * 1024;

/// Pick a picture and store it as a cover (shrunk into `covers/`); `Ok(None)` when cancelled or
/// unreadable. Returns the `owcover:<hash>` reference.
pub async fn pick_cover_image() -> ApiResult<Option<String>> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_path_buf();
    let too_big = tokio::fs::metadata(&path)
        .await
        .map_or(true, |m| m.len() > MAX_COVER_BYTES);
    if too_big {
        return Ok(None);
    }
    let b = backend_ref()?;
    let covers = b.media.covers_dir();
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| ApiError::from(MediaError::from(e)))?;
    let hash = tokio::task::spawn_blocking(move || local_tags::save_cover(&bytes, &covers))
        .await
        .map_err(|e| ApiError::from(MediaError::Io(e.to_string())))?;
    Ok(hash.map(|h| format!("{}{h}", local_tags::COVER_SCHEME)))
}

/// Pick a folder and add every audio file under it (sub-folders included).
pub async fn import_folder() -> ApiResult<Vec<Track>> {
    let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
        return Ok(vec![]);
    };
    let root = folder.path().to_path_buf();
    let paths = tokio::task::spawn_blocking(move || local_tags::scan_folder(&root, is_audio_file))
        .await
        .map_err(|e| ApiError::from(MediaError::Io(e.to_string())))?;
    add_local_files(paths).await
}

/// Open a native picker and register the chosen audio files as local tracks (empty if cancelled).
pub async fn import_audio() -> ApiResult<Vec<Track>> {
    let picked = rfd::AsyncFileDialog::new()
        .add_filter("Audio", &LOCAL_EXTENSIONS)
        .pick_files()
        .await;
    match picked {
        Some(files) => {
            add_local_files(files.into_iter().map(|f| f.path().to_path_buf()).collect()).await
        }
        None => Ok(vec![]),
    }
}
