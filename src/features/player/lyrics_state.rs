//! Fetching lyrics. Nothing is requested unless the user opted in, and only when a lyrics view
//! is on screen (views call [`request`] when they mount or the song changes).

use std::sync::Arc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::backend::api;
use crate::backend::lyrics::LyricsQuery;
use crate::backend::Coded;
use crate::model::{Id, Lyrics, Track};
use crate::store::{self, SettingsChange};

#[derive(Clone, Debug, PartialEq)]
pub enum LyricsView {
    Idle,
    /// The opt-in is off: nothing was sent anywhere.
    Disabled,
    Loading,
    Ready(Arc<Lyrics>),
    NotFound,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LyricsEntry {
    pub track_id: Option<Id>,
    pub view: LyricsView,
}

pub static LYRICS: GlobalSignal<LyricsEntry> = Signal::global(|| LyricsEntry {
    track_id: None,
    view: LyricsView::Idle,
});
/// Bumped when the opt-in changes so views re-read the setting.
pub static LYRICS_SETTING_REV: GlobalSignal<u64> = Signal::global(|| 0);

pub fn lyrics_enabled() -> bool {
    let _ = *LYRICS_SETTING_REV.read();
    store::settings().get().lyrics_enabled
}

fn set(track_id: &Id, view: LyricsView) {
    if LYRICS.peek().track_id.as_ref() == Some(track_id) {
        LYRICS.write().view = view;
    }
}

/// The id when the track streams from YouTube (not a local file or a SoundCloud song).
fn youtube_id(track: &Track) -> Option<&str> {
    let id = track.id.as_str();
    (crate::backend::media::video_key(id) && !id.starts_with("sc-")).then_some(id)
}

/// Whether a lookup is worth making for this track (local files without artist are skipped so
/// file names are never sent anywhere).
fn lookup_allowed(track: &Track) -> bool {
    !(track.is_local() && track.artists.is_empty())
}

/// Make sure lyrics for `track` are loaded (or loading). `force` retries after a failure.
pub fn request(track: &Track, force: bool) {
    let enabled = store::settings().get().lyrics_enabled;
    let current = LYRICS.peek().clone();
    let same = current.track_id.as_ref() == Some(&track.id);
    let settled = match current.view {
        LyricsView::Loading | LyricsView::Ready(_) | LyricsView::NotFound => enabled,
        LyricsView::Disabled => !enabled,
        LyricsView::Failed(_) => !force,
        LyricsView::Idle => false,
    };
    if same && settled {
        return;
    }
    let view = if track.is_local() {
        // A `.lrc` next to the file is read from disk whatever the online setting says.
        LyricsView::Loading
    } else if !enabled {
        LyricsView::Disabled
    } else if !lookup_allowed(track) {
        LyricsView::NotFound
    } else {
        LyricsView::Loading
    };
    let fetch = view == LyricsView::Loading;
    *LYRICS.write() = LyricsEntry {
        track_id: Some(track.id.clone()),
        view,
    };
    if fetch {
        spawn_forever(fetch_task(track.clone()));
    }
}

async fn fetch_task(track: Track) {
    if track.is_local() {
        if let Ok(Some(l)) = api::local_lyrics(&track.id).await {
            set(&track.id, LyricsView::Ready(Arc::new(l)));
            return;
        }
        if !store::settings().get().lyrics_enabled {
            set(&track.id, LyricsView::Disabled);
            return;
        }
        if !lookup_allowed(&track) {
            set(&track.id, LyricsView::NotFound);
            return;
        }
    }
    let (artist, album, duration) = query_parts(&track);
    let query = LyricsQuery {
        title: &track.title,
        artist: &artist,
        album: album.as_deref(),
        duration_secs: duration,
        video_id: youtube_id(&track),
    };
    let view = match api::lyrics(true, &query).await {
        Ok(Some(l)) if !l.is_empty() => LyricsView::Ready(Arc::new(l)),
        Ok(_) => LyricsView::NotFound,
        Err(e) => LyricsView::Failed(e.user_message()),
    };
    set(&track.id, view);
}

/// Artist label, album and duration (s) of a lookup, owned so a query can borrow them.
fn query_parts(track: &Track) -> (String, Option<String>, Option<f64>) {
    let album = track.album.as_ref().map(|a| a.title.to_string());
    let duration = (track.duration_ms > 0).then(|| track.duration_ms as f64 / 1000.0);
    (track.artists_label(), album, duration)
}

/// The "Translate" toggle of the lyrics pane (off at every start).
pub static TRANSLATE: GlobalSignal<bool> = Signal::global(|| false);
/// Translated lines of one song: (track, language, lines). Empty lines while it loads.
type Translation = (Id, &'static str, Arc<Vec<String>>);
static TRANSLATION: GlobalSignal<Option<Translation>> = Signal::global(|| None);

/// The translation of `track_id` in the UI language, when ready and matching `lines` lines.
pub fn translation_for(track_id: &Id, lines: usize) -> Option<Arc<Vec<String>>> {
    if !*TRANSLATE.read() {
        return None;
    }
    let lang = crate::i18n::language().code();
    match TRANSLATION.read().as_ref() {
        Some((id, l, t)) if id == track_id && *l == lang && t.len() == lines => Some(t.clone()),
        _ => None,
    }
}

/// Whether the translation slot holds (or is loading) `track_id` in `lang`.
fn pending_or_done(track_id: &Id, lang: &str) -> bool {
    TRANSLATION
        .peek()
        .as_ref()
        .is_some_and(|(id, l, _)| id == track_id && *l == lang)
}

/// Translate `lines` of `track` into the UI language (once per song and language).
pub fn request_translation(track: &Track, lines: Vec<String>) {
    let lang = crate::i18n::language().code();
    if pending_or_done(&track.id, lang) {
        return;
    }
    *TRANSLATION.write() = Some((track.id.clone(), lang, Arc::new(vec![])));
    let track = track.clone();
    spawn_forever(async move {
        let (artist, album, duration) = query_parts(&track);
        let query = LyricsQuery {
            title: &track.title,
            artist: &artist,
            album: album.as_deref(),
            duration_secs: duration,
            video_id: youtube_id(&track),
        };
        match api::translate_lyrics(&query, lines, lang).await {
            Ok(t) => {
                if pending_or_done(&track.id, lang) {
                    *TRANSLATION.write() = Some((track.id.clone(), lang, Arc::new(t)));
                }
            }
            Err(_) => {
                if pending_or_done(&track.id, lang) {
                    *TRANSLATION.write() = None;
                    *TRANSLATE.write() = false;
                }
                crate::ui::toast_error(crate::i18n::t("lyrics.translate.failed"));
            }
        }
    });
}

/// The user pressed "Etkinleştir": flip the setting and look the lyrics up.
pub fn enable(track: Option<&Track>) {
    let result = store::settings().update(|s| s.apply(SettingsChange::LyricsEnabled(true)));
    if result.is_err() {
        crate::ui::toast_error(crate::i18n::t("error.store.write"));
    }
    *LYRICS_SETTING_REV.write() += 1;
    if let Some(track) = track {
        request(track, true);
    }
}
