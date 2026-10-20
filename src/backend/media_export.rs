//! Exporting downloads: copies `downloads/<id>.audio` into a folder the user picked as
//! `Artist - Title.m4a`, tagged with title, artist, album and cover. Older downloads that are not
//! MP4 keep their own container (`.webm`) and are copied untagged.
use super::{video_key, Media, MediaError};
use crate::model::Track;
use lofty::{
    config::WriteOptions,
    file::TaggedFileExt,
    picture::{MimeType, Picture, PictureType},
    probe::Probe,
    tag::{Accessor, Tag, TagExt, TagType},
};
use std::path::{Path, PathBuf};

/// Longest file stem written (Windows paths stay well under their limit).
const MAX_STEM: usize = 120;

/// File extension for a stored download's mime type.
pub(crate) fn extension(mime: &str) -> &'static str {
    match mime {
        "audio/mp4" => "m4a",
        "audio/webm" => "webm",
        "audio/mpeg" => "mp3",
        _ => "audio",
    }
}

/// `s` without characters Windows, macOS or Linux reject in a file name, trimmed and bounded.
pub(crate) fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(MAX_STEM)
        .collect();
    // Windows drops trailing dots and spaces, which would make two names collide.
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).to_owned();
    if trimmed.is_empty() {
        "track".into()
    } else {
        trimmed
    }
}

/// "Artist - Title", or the title alone when the artist is unknown.
pub(crate) fn file_stem(track: &Track) -> String {
    let artist = track.artists_label();
    if artist.trim().is_empty() {
        sanitize(&track.title)
    } else {
        sanitize(&format!("{artist} - {}", track.title))
    }
}

/// `dir/stem.ext`, or `dir/stem (2).ext`, `(3)`... when that name is taken.
pub(crate) fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Write title, artist, album and an optional JPEG cover into the MP4 file at `path`.
pub(crate) fn write_tags(path: &Path, track: &Track, cover: Option<Vec<u8>>) -> Result<(), String> {
    let mut tagged = Probe::open(path)
        .and_then(|p| p.read())
        .map_err(|e| e.to_string())?;
    if tagged.tag(TagType::Mp4Ilst).is_none() {
        tagged.insert_tag(Tag::new(TagType::Mp4Ilst));
    }
    let tag = tagged
        .tag_mut(TagType::Mp4Ilst)
        .ok_or_else(|| "no mp4 tag".to_owned())?;
    tag.set_title(track.title.to_string());
    let artist = track.artists_label();
    if !artist.is_empty() {
        tag.set_artist(artist);
    }
    if let Some(album) = track.album.as_ref().filter(|a| !a.title.is_empty()) {
        tag.set_album(album.title.to_string());
    }
    if let Some(data) = cover {
        tag.push_picture(
            Picture::unchecked(data)
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
    }
    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| e.to_string())
}

/// Any decodable image re-encoded as JPEG (MP4 covers must be JPEG or PNG; thumbnails are WebP).
fn to_jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img.to_rgb8())
        .ok()?;
    Some(out)
}

impl Media {
    /// Copy the download of `key` into `dir`; returns the written file. MP4 downloads are tagged
    /// (a cover that cannot be fetched is skipped, never fatal).
    pub async fn export_download(&self, key: &str, dir: &Path) -> Result<PathBuf, MediaError> {
        if !video_key(key) {
            return Err(MediaError::InvalidTrackId);
        }
        let (audio, mime) = self
            .downloaded_file(key)
            .await
            .ok_or(MediaError::Unavailable)?;
        let track = self.downloaded_track(key).await.unwrap_or_default();
        let ext = extension(&mime);
        let dest = free_path(dir, &file_stem(&track), ext);
        tokio::fs::copy(&audio, &dest).await?;
        if ext != "m4a" {
            return Ok(dest);
        }
        let cover = match track.cover.as_ref() {
            Some(c) => super::super::palette_net::download(&c.url)
                .await
                .ok()
                .and_then(|b| to_jpeg(&b)),
            None => None,
        };
        let (path, t) = (dest.clone(), track);
        let tagged = tokio::task::spawn_blocking(move || write_tags(&path, &t, cover))
            .await
            .map_err(|e| MediaError::Io(e.to_string()))?;
        if let Err(e) = tagged {
            // A half-written copy is worse than none.
            let _ = tokio::fs::remove_file(&dest).await;
            return Err(MediaError::Io(e));
        }
        Ok(dest)
    }
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("media_export_tests.rs");
}
