//! Real metadata for local music files: tags (title, artists, album), duration and the embedded
//! cover, read with `lofty`. Covers are shrunk to a small JPEG in `covers/<hash>.jpg` and the
//! track only stores `owcover:<hash>`; `api::cover_src` turns that into a proxy URL at render time,
//! so `library.json` stays small however many files are added.
use crate::model::{AlbumRef, ArtistRef, Cover, Track};
use lofty::{
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Prefix of a stored local cover reference.
pub const COVER_SCHEME: &str = "owcover:";
/// Longest side of a stored cover, in px.
const COVER_PX: u32 = 320;
/// Embedded pictures larger than this are ignored (a broken tag must not eat memory).
const MAX_PICTURE_BYTES: usize = 16 * 1024 * 1024;
/// Folder scans stop after this many audio files.
pub const MAX_SCAN_FILES: usize = 5000;
const MAX_SCAN_DEPTH: usize = 12;

/// Metadata read from one file. Missing tags stay `None` / empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalMeta {
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: u64,
    pub cover_hash: Option<String>,
}

/// A cover hash as stored: 16 lowercase hex digits.
pub fn valid_cover_hash(h: &str) -> bool {
    h.len() == 16
        && h.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn clean(s: &str) -> Option<String> {
    let t = s.trim().trim_matches('\0').trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// Tag artist fields join several artists with `;`, `/` or NUL; commas are kept (band names).
pub fn split_artists(s: &str) -> Vec<String> {
    s.split([';', '\0'])
        .flat_map(|part| part.split(" / "))
        .filter_map(clean)
        .collect()
}

/// "Artist - Title" file names, the common convention when a file has no tags.
pub fn from_file_name(stem: &str) -> (Option<String>, String) {
    match stem.split_once(" - ") {
        Some((a, t)) if clean(a).is_some() && clean(t).is_some() => (clean(a), t.trim().to_owned()),
        _ => (None, stem.trim().to_owned()),
    }
}

/// Shrink `bytes` (any decodable image) to a JPEG thumbnail in `dir`; returns its hash. The hash
/// is of the source picture, so every track of an album shares one file.
pub fn save_cover(bytes: &[u8], dir: &Path) -> Option<String> {
    if bytes.is_empty() || bytes.len() > MAX_PICTURE_BYTES {
        return None;
    }
    let digest = Sha256::digest(bytes);
    let hash: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
    let path = dir.join(format!("{hash}.jpg"));
    if path.is_file() {
        return Some(hash);
    }
    // Untrusted pictures: a tiny file may declare a huge canvas, so cap dimensions and memory.
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().ok()?;
    let thumb = img.thumbnail(COVER_PX, COVER_PX).to_rgb8();
    std::fs::create_dir_all(dir).ok()?;
    let tmp = dir.join(format!("{hash}.{}.tmp", uuid::Uuid::new_v4()));
    let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp).ok()?);
    let encoded =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode_image(&thumb);
    drop(out);
    if encoded.is_err() || std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    Some(hash)
}

/// Read tags, duration and cover of `path` (blocking). Unreadable files give empty metadata.
pub fn read_meta(path: &Path, covers_dir: &Path) -> LocalMeta {
    let Ok(tagged) = Probe::open(path).and_then(|p| p.read()) else {
        return LocalMeta::default();
    };
    let duration_ms = u64::try_from(tagged.properties().duration().as_millis()).unwrap_or(0);
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return LocalMeta {
            duration_ms,
            ..LocalMeta::default()
        };
    };
    let cover_hash = tag
        .pictures()
        .first()
        .and_then(|p| save_cover(p.data(), covers_dir));
    LocalMeta {
        title: tag.title().as_deref().and_then(clean),
        artists: tag
            .artist()
            .as_deref()
            .map(split_artists)
            .unwrap_or_default(),
        album: tag.album().as_deref().and_then(clean),
        duration_ms,
        cover_hash,
    }
}

/// The library track for a registered local file.
pub fn to_track(id: &str, path: &Path, meta: LocalMeta) -> Track {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let (name_artist, name_title) = from_file_name(&stem);
    let artists = if meta.artists.is_empty() {
        name_artist.into_iter().collect()
    } else {
        meta.artists
    };
    Track {
        id: id.into(),
        title: meta.title.unwrap_or(name_title).into(),
        artists: artists
            .into_iter()
            .map(|name| ArtistRef {
                id: "".into(),
                name: name.into(),
            })
            .collect(),
        album: meta.album.map(|title| AlbumRef {
            id: "".into(),
            title: title.into(),
        }),
        duration_ms: meta.duration_ms,
        cover: meta.cover_hash.map(|h| Cover {
            url: format!("{COVER_SCHEME}{h}").into(),
            width: COVER_PX,
            height: COVER_PX,
        }),
        explicit: false,
    }
}

/// Every audio file under `root` (depth- and count-limited, links not followed), sorted.
pub fn scan_folder(root: &Path, is_audio: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut found = vec![];
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            let path = e.path();
            if kind.is_dir() && depth < MAX_SCAN_DEPTH {
                stack.push((path, depth + 1));
            } else if kind.is_file() && is_audio(&path) {
                found.push(path);
                if found.len() >= MAX_SCAN_FILES {
                    found.sort();
                    return found;
                }
            }
        }
    }
    found.sort();
    found
}

/// The `.lrc` file next to `audio` (same name), if there is one.
pub fn sidecar_lrc(audio: &Path) -> Option<PathBuf> {
    let lrc = audio.with_extension("lrc");
    lrc.is_file().then_some(lrc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ow-tags-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A short, valid 8 kHz mono 16-bit WAV of `seconds`.
    fn wav(path: &Path, seconds: u32) {
        let samples = 8000 * seconds;
        let data_len = samples * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8000u32.to_le_bytes());
        b.extend_from_slice(&16000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        b.resize(b.len() + data_len as usize, 0);
        std::fs::write(path, b).unwrap();
    }

    #[test]
    fn artists_split_on_tag_separators_but_keep_commas() {
        assert_eq!(
            split_artists("Tarkan; Sezen Aksu"),
            ["Tarkan", "Sezen Aksu"]
        );
        assert_eq!(split_artists("A / B\0C"), ["A", "B", "C"]);
        assert_eq!(split_artists("Earth, Wind & Fire"), ["Earth, Wind & Fire"]);
        assert!(split_artists(" ; ").is_empty());
    }

    #[test]
    fn file_names_give_artist_and_title() {
        assert_eq!(
            from_file_name("Tarkan - Kuzu Kuzu"),
            (Some("Tarkan".into()), "Kuzu Kuzu".into())
        );
        assert_eq!(from_file_name("Kuzu Kuzu"), (None, "Kuzu Kuzu".into()));
        assert_eq!(from_file_name(" - x"), (None, "- x".into()));
    }

    #[test]
    fn untagged_wav_reads_duration_and_falls_back_to_the_file_name() {
        let d = tmp();
        let path = d.join("Duman - Senden Daha Güzel.wav");
        wav(&path, 3);
        let meta = read_meta(&path, &d.join("covers"));
        assert!(
            (2900..=3100).contains(&meta.duration_ms),
            "{}",
            meta.duration_ms
        );
        let t = to_track("local-1", &path, meta);
        assert_eq!(t.title.as_ref(), "Senden Daha Güzel");
        assert_eq!(t.artists[0].name.as_ref(), "Duman");
        assert!(t.cover.is_none() && t.album.is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn tags_win_over_the_file_name() {
        let meta = LocalMeta {
            title: Some("Şımarık".into()),
            artists: vec!["Tarkan".into()],
            album: Some("Ölürüm Sana".into()),
            duration_ms: 1000,
            cover_hash: Some("0123456789abcdef".into()),
        };
        let t = to_track("local-1", Path::new("C:/m/track01.mp3"), meta);
        assert_eq!(
            (t.title.as_ref(), t.artists[0].name.as_ref()),
            ("Şımarık", "Tarkan")
        );
        assert_eq!(t.album.unwrap().title.as_ref(), "Ölürüm Sana");
        assert_eq!(t.cover.unwrap().url.as_ref(), "owcover:0123456789abcdef");
    }

    #[test]
    fn covers_are_shrunk_shared_and_garbage_is_ignored() {
        let d = tmp();
        let img = image::RgbImage::from_pixel(800, 600, image::Rgb([200, 30, 30]));
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let bytes = png.into_inner();
        let a = save_cover(&bytes, &d).unwrap();
        let b = save_cover(&bytes, &d).unwrap();
        assert_eq!(a, b);
        assert!(valid_cover_hash(&a));
        let saved = image::open(d.join(format!("{a}.jpg"))).unwrap();
        assert_eq!((saved.width(), saved.height()), (320, 240));
        assert!(save_cover(b"not an image", &d).is_none());
        assert!(save_cover(&[], &d).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hashes_are_strict() {
        assert!(valid_cover_hash("0123456789abcdef"));
        for bad in [
            "0123456789ABCDEF",
            "0123",
            "../../etc/passwd",
            "0123456789abcdeg",
        ] {
            assert!(!valid_cover_hash(bad), "{bad}");
        }
    }

    #[test]
    fn folder_scan_finds_nested_audio_and_lrc_sidecars() {
        let d = tmp();
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::write(d.join("a/one.mp3"), b"x").unwrap();
        std::fs::write(d.join("a/b/two.flac"), b"x").unwrap();
        std::fs::write(d.join("a/b/two.lrc"), b"[00:01.00]hi").unwrap();
        std::fs::write(d.join("a/notes.txt"), b"x").unwrap();
        let is_audio = |p: &Path| p.extension().is_some_and(|e| e == "mp3" || e == "flac");
        let found = scan_folder(&d, is_audio);
        assert_eq!(found.len(), 2);
        assert!(sidecar_lrc(&d.join("a/b/two.flac")).is_some());
        assert!(sidecar_lrc(&d.join("a/one.mp3")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
