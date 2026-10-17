//! Typed backend errors. No user-facing prose lives here: every error exposes a machine `code()`
//! that is also its i18n key (present in `assets/locales/*.json`). The UI shows
//! `err.user_message()` (translated) or maps `err.code()` itself. `Display` is English and meant
//! for logs only.
use std::fmt;

/// Implemented by every backend error.
pub trait Coded {
    /// Stable machine code, identical to the i18n key of the user message.
    fn code(&self) -> &'static str;
    /// Values for `{name}` placeholders in the translated message.
    fn args(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }
    /// Translated, jargon-free message in the current UI language.
    fn user_message(&self) -> String {
        let owned = self.args();
        let refs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
        crate::i18n::t_args(self.code(), &refs)
    }
}

macro_rules! impl_display {
    ($t:ty) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.detail() {
                    Some(d) => write!(f, "{} ({d})", self.code()),
                    None => f.write_str(self.code()),
                }
            }
        }
        impl std::error::Error for $t {}
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    /// Query or id failed validation before any request was made.
    InvalidRequest,
    UnknownSection,
    /// The catalog could not be reached (offline, blocked, timed out).
    Network(String),
    /// The catalog answered with something we could not read.
    BadResponse(String),
}
impl Coded for CatalogError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest | Self::UnknownSection => "error.invalid_request",
            Self::Network(_) => "error.network",
            Self::BadResponse(_) => "error.bad_response",
        }
    }
}
impl CatalogError {
    fn detail(&self) -> Option<&str> {
        match self {
            Self::Network(d) | Self::BadResponse(d) => Some(d),
            _ => None,
        }
    }
}
impl_display!(CatalogError);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaError {
    InvalidTrackId,
    /// No direct stream and no yt-dlp available to fall back on.
    YtDlpMissing,
    Timeout,
    /// The track cannot be streamed (removed, region/age restricted, fallback failed).
    Unavailable,
    /// A stream URL outside the allowed hosts was rejected.
    UntrustedSource,
    BadResponse,
    /// A proxy ticket expired.
    SessionExpired,
    Network(String),
    Io(String),
}
impl Coded for MediaError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidTrackId => "error.media.invalid_track",
            Self::YtDlpMissing => "error.media.ytdlp_missing",
            Self::Timeout => "error.media.timeout",
            Self::Unavailable => "error.media.unavailable",
            Self::UntrustedSource => "error.media.untrusted",
            Self::BadResponse => "error.bad_response",
            Self::SessionExpired => "error.media.expired",
            Self::Network(_) => "error.network",
            Self::Io(_) => "error.io",
        }
    }
}
impl MediaError {
    fn detail(&self) -> Option<&str> {
        match self {
            Self::Network(d) | Self::Io(d) => Some(d),
            _ => None,
        }
    }
}
impl From<std::io::Error> for MediaError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
impl_display!(MediaError);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// Not a Spotify or YouTube playlist link.
    BadLink,
    /// A valid link, but to a track/album/artist/video rather than a playlist.
    NotAPlaylist,
    PrivateOrEmpty,
    Offline,
    NothingMatched,
    /// Not a failure: only the first `limit` tracks could be read.
    Truncated {
        limit: usize,
    },
    /// The source page had an unexpected layout.
    Unreadable,
}
impl Coded for ImportError {
    fn code(&self) -> &'static str {
        match self {
            Self::BadLink => "import.error.bad_link",
            Self::NotAPlaylist => "import.error.not_a_playlist",
            Self::PrivateOrEmpty => "import.error.private_or_empty",
            Self::Offline => "import.error.offline",
            Self::NothingMatched => "import.error.nothing_matched",
            Self::Truncated { .. } => "import.error.truncated",
            Self::Unreadable => "import.error.unreadable",
        }
    }
    fn args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Truncated { limit } => vec![("limit", limit.to_string())],
            _ => vec![],
        }
    }
}
impl ImportError {
    fn detail(&self) -> Option<String> {
        match self {
            Self::Truncated { limit } => Some(format!("limit={limit}")),
            _ => None,
        }
    }
}
impl From<CatalogError> for ImportError {
    fn from(e: CatalogError) -> Self {
        match e {
            CatalogError::Network(_) => Self::Offline,
            _ => Self::Unreadable,
        }
    }
}
impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.detail() {
            Some(d) => write!(f, "{} ({d})", self.code()),
            None => f.write_str(self.code()),
        }
    }
}
impl std::error::Error for ImportError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LyricsError {
    /// The user has not opted in; no request was made.
    Disabled,
    InvalidInput,
    Network(String),
    Unavailable,
}
impl Coded for LyricsError {
    fn code(&self) -> &'static str {
        match self {
            Self::Disabled => "lyrics.error.disabled",
            Self::InvalidInput => "error.invalid_request",
            Self::Network(_) => "error.network",
            Self::Unavailable => "lyrics.error.unavailable",
        }
    }
}
impl LyricsError {
    fn detail(&self) -> Option<&str> {
        match self {
            Self::Network(d) => Some(d),
            _ => None,
        }
    }
}
impl_display!(LyricsError);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YtDlpError {
    Network(String),
    /// GitHub answered with an unexpected status or page.
    BadRelease(String),
    /// The published checksum list does not mention the file.
    ChecksumMissing,
    ChecksumMismatch,
    /// The downloaded or installed binary did not run.
    BrokenBinary,
    Timeout,
    Io(String),
}
impl Coded for YtDlpError {
    fn code(&self) -> &'static str {
        match self {
            Self::Network(_) => "error.network",
            Self::BadRelease(_) => "ytdlp.error.bad_release",
            Self::ChecksumMissing | Self::ChecksumMismatch => "ytdlp.error.checksum",
            Self::BrokenBinary => "ytdlp.error.broken",
            Self::Timeout => "error.media.timeout",
            Self::Io(_) => "error.io",
        }
    }
}
impl YtDlpError {
    fn detail(&self) -> Option<&str> {
        match self {
            Self::Network(d) | Self::BadRelease(d) | Self::Io(d) => Some(d),
            _ => None,
        }
    }
}
impl From<std::io::Error> for YtDlpError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
impl_display!(YtDlpError);

/// Union returned by [`crate::backend::api`] functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    Catalog(CatalogError),
    Media(MediaError),
    Import(ImportError),
    Lyrics(LyricsError),
    YtDlp(YtDlpError),
    /// Backend not started, task panicked, data dir unusable.
    Internal(String),
}
impl Coded for ApiError {
    fn code(&self) -> &'static str {
        match self {
            Self::Catalog(e) => e.code(),
            Self::Media(e) => e.code(),
            Self::Import(e) => e.code(),
            Self::Lyrics(e) => e.code(),
            Self::YtDlp(e) => e.code(),
            Self::Internal(_) => "error.internal",
        }
    }
    fn args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Import(e) => e.args(),
            _ => vec![],
        }
    }
}
impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(e) => e.fmt(f),
            Self::Media(e) => e.fmt(f),
            Self::Import(e) => e.fmt(f),
            Self::Lyrics(e) => e.fmt(f),
            Self::YtDlp(e) => e.fmt(f),
            Self::Internal(d) => write!(f, "error.internal ({d})"),
        }
    }
}
impl std::error::Error for ApiError {}
macro_rules! api_from {
    ($($t:ty => $v:ident),*) => {$(
        impl From<$t> for ApiError { fn from(e: $t) -> Self { Self::$v(e) } }
    )*};
}
api_from!(CatalogError => Catalog, MediaError => Media, ImportError => Import,
          LyricsError => Lyrics, YtDlpError => YtDlp);

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
pub(crate) fn sample_errors() -> Vec<Box<dyn Coded>> {
    vec![
        Box::new(CatalogError::InvalidRequest),
        Box::new(CatalogError::UnknownSection),
        Box::new(CatalogError::Network("x".into())),
        Box::new(CatalogError::BadResponse("x".into())),
        Box::new(MediaError::InvalidTrackId),
        Box::new(MediaError::YtDlpMissing),
        Box::new(MediaError::Timeout),
        Box::new(MediaError::Unavailable),
        Box::new(MediaError::UntrustedSource),
        Box::new(MediaError::BadResponse),
        Box::new(MediaError::SessionExpired),
        Box::new(MediaError::Network("x".into())),
        Box::new(MediaError::Io("x".into())),
        Box::new(ImportError::BadLink),
        Box::new(ImportError::NotAPlaylist),
        Box::new(ImportError::PrivateOrEmpty),
        Box::new(ImportError::Offline),
        Box::new(ImportError::NothingMatched),
        Box::new(ImportError::Truncated { limit: 100 }),
        Box::new(ImportError::Unreadable),
        Box::new(LyricsError::Disabled),
        Box::new(LyricsError::InvalidInput),
        Box::new(LyricsError::Network("x".into())),
        Box::new(LyricsError::Unavailable),
        Box::new(YtDlpError::Network("x".into())),
        Box::new(YtDlpError::BadRelease("x".into())),
        Box::new(YtDlpError::ChecksumMissing),
        Box::new(YtDlpError::ChecksumMismatch),
        Box::new(YtDlpError::BrokenBinary),
        Box::new(YtDlpError::Timeout),
        Box::new(YtDlpError::Io("x".into())),
        Box::new(ApiError::Internal("x".into())),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_dotted_i18n_keys() {
        for e in sample_errors() {
            assert!(e.code().contains('.'), "{}", e.code());
        }
    }

    #[test]
    fn api_error_forwards_code_and_args() {
        let e: ApiError = ImportError::Truncated { limit: 100 }.into();
        assert_eq!(e.code(), "import.error.truncated");
        assert_eq!(e.args(), vec![("limit", "100".to_string())]);
        assert_eq!(e.to_string(), "import.error.truncated (limit=100)");
    }

    #[test]
    fn catalog_network_maps_to_offline_import_error() {
        assert_eq!(
            ImportError::from(CatalogError::Network("x".into())),
            ImportError::Offline
        );
        assert_eq!(
            ImportError::from(CatalogError::InvalidRequest),
            ImportError::Unreadable
        );
    }
}
