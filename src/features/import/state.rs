//! Pure logic of the import dialog: which step follows which, link validation and clipboard link
//! detection. No UI and no network, so everything here is unit tested.

use crate::backend::error::ImportError;
use crate::backend::import::{parse_link, parse_source, Source};

/// Which service a link (or the user's choice) belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Spotify,
    Youtube,
}

impl Kind {
    pub fn of(source: &Source) -> Kind {
        match source {
            Source::Spotify(_) => Kind::Spotify,
            // Shared lists are YouTube Music songs.
            Source::Youtube(_) | Source::Shared(_) => Kind::Youtube,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Source,
    Link,
    /// Reading the list (nothing is saved).
    Reading,
    Preview,
    /// Matching songs; the only step where closing would throw work away.
    Matching,
    /// The native file picker is open or the chosen files are being registered.
    Adding,
    Result,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Pick,
    PickFiles,
    FilesDone,
    FilesNone,
    Back,
    Submit,
    PrepareOk,
    PrepareErr,
    Confirm,
    RunOk,
    RunErr,
}

/// The next step for `event`; unrelated events leave the step unchanged.
pub fn transition(step: Step, event: Event) -> Step {
    use Event::*;
    use Step::*;
    match (step, event) {
        (Source, Pick) => Link,
        (Source, PickFiles) => Adding,
        (Adding, FilesDone) => Result,
        (Adding, FilesNone) => Source,
        (Adding, RunErr) => Failed,
        (Link, Back) => Source,
        (Link, Submit) => Reading,
        (Reading, PrepareOk) => Preview,
        (Reading, PrepareErr | Back) => Link,
        (Preview, Back) => Link,
        (Preview, Confirm) => Matching,
        (Matching, RunOk) => Result,
        (Matching, RunErr) => Failed,
        (Failed, Back) => Preview,
        (s, _) => s,
    }
}

/// Closing needs a confirmation only while matching is running.
pub fn needs_close_confirm(step: Step) -> bool {
    step == Step::Matching
}

/// DOM id of the control that should hold focus on `step`.
pub fn focus_target(step: Step) -> &'static str {
    match step {
        Step::Source => "ow-imp-first-source",
        Step::Link => "ow-imp-link",
        Step::Reading | Step::Matching | Step::Adding => "ow-imp-cancel",
        Step::Preview => "ow-imp-confirm",
        Step::Result | Step::Failed => "ow-imp-primary",
    }
}

/// Result of checking what the user typed or pasted. Runs on every keystroke, no network.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkCheck {
    Empty,
    Valid(Source),
    Invalid(ImportError),
}

pub fn check_link(input: &str) -> LinkCheck {
    if input.trim().is_empty() {
        return LinkCheck::Empty;
    }
    match parse_link(input) {
        Ok(source) => LinkCheck::Valid(source),
        Err(e) => LinkCheck::Invalid(e),
    }
}

/// A supported playlist link found in clipboard text: the source and the exact token to prefill.
pub fn find_link(text: &str) -> Option<(Source, String)> {
    let trimmed = text.trim();
    if let Some(source) = parse_source(trimmed) {
        return Some((source, trimmed.to_owned()));
    }
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| matches!(c, '<' | '>' | '"' | '\'' | ',' | ')' | '(')))
        .filter(|w| {
            ["spotify", "youtube", "youtu.be", "openwave"]
                .iter()
                .any(|k| w.contains(k))
        })
        .find_map(|w| parse_source(w).map(|s| (s, w.to_owned())))
}

/// True when the audio files should be taken from the file picker rather than a link.
pub fn is_audio_path(path: &std::path::Path) -> bool {
    const EXT: [&str; 7] = ["mp3", "m4a", "aac", "flac", "wav", "ogg", "opus"];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXT.contains(&e.to_ascii_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const SP: &str = "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M";
    const YT: &str = "https://music.youtube.com/playlist?list=PLabc123_-xyz";

    #[test]
    fn happy_path_walks_every_step() {
        let mut s = Step::Source;
        for (ev, want) in [
            (Event::Pick, Step::Link),
            (Event::Submit, Step::Reading),
            (Event::PrepareOk, Step::Preview),
            (Event::Confirm, Step::Matching),
            (Event::RunOk, Step::Result),
        ] {
            s = transition(s, ev);
            assert_eq!(s, want);
        }
    }

    #[test]
    fn errors_and_backs_return_to_a_sane_step() {
        assert_eq!(transition(Step::Reading, Event::PrepareErr), Step::Link);
        assert_eq!(transition(Step::Reading, Event::Back), Step::Link);
        assert_eq!(transition(Step::Matching, Event::RunErr), Step::Failed);
        assert_eq!(transition(Step::Failed, Event::Back), Step::Preview);
        assert_eq!(transition(Step::Link, Event::Back), Step::Source);
    }

    #[test]
    fn picking_files_skips_the_link_steps() {
        assert_eq!(transition(Step::Source, Event::PickFiles), Step::Adding);
        assert_eq!(transition(Step::Adding, Event::FilesDone), Step::Result);
        assert_eq!(transition(Step::Adding, Event::FilesNone), Step::Source);
        assert_eq!(transition(Step::Adding, Event::RunErr), Step::Failed);
    }

    #[test]
    fn unrelated_events_do_nothing() {
        assert_eq!(transition(Step::Source, Event::Confirm), Step::Source);
        assert_eq!(transition(Step::Result, Event::Submit), Step::Result);
        assert_eq!(transition(Step::Matching, Event::Back), Step::Matching);
    }

    #[test]
    fn only_matching_asks_before_closing() {
        for s in [
            Step::Source,
            Step::Link,
            Step::Reading,
            Step::Preview,
            Step::Adding,
            Step::Result,
            Step::Failed,
        ] {
            assert!(!needs_close_confirm(s), "{s:?}");
        }
        assert!(needs_close_confirm(Step::Matching));
    }

    #[test]
    fn every_step_has_a_focus_target() {
        assert_eq!(focus_target(Step::Link), "ow-imp-link");
        assert!(focus_target(Step::Preview).starts_with("ow-imp-"));
    }

    #[test]
    fn link_check_distinguishes_empty_valid_and_each_error() {
        assert_eq!(check_link("   "), LinkCheck::Empty);
        assert!(matches!(
            check_link(SP),
            LinkCheck::Valid(Source::Spotify(_))
        ));
        assert!(matches!(
            check_link(YT),
            LinkCheck::Valid(Source::Youtube(_))
        ));
        assert_eq!(
            check_link("hello world"),
            LinkCheck::Invalid(ImportError::BadLink)
        );
        assert_eq!(
            check_link("https://example.com/playlist/abc"),
            LinkCheck::Invalid(ImportError::BadLink)
        );
        assert_eq!(
            check_link("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"),
            LinkCheck::Invalid(ImportError::NotAPlaylist)
        );
        assert_eq!(
            check_link("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            LinkCheck::Invalid(ImportError::NotAPlaylist)
        );
    }

    #[test]
    fn clipboard_detection_finds_links_inside_text() {
        let (src, token) = find_link(&format!("  {SP}\n")).expect("plain");
        assert_eq!(Kind::of(&src), Kind::Spotify);
        assert_eq!(token, SP);
        let (src, token) = find_link(&format!("Dinle: <{YT}> güzel")).expect("embedded");
        assert_eq!(Kind::of(&src), Kind::Youtube);
        assert_eq!(token, YT);
    }

    #[test]
    fn clipboard_detection_ignores_everything_else() {
        assert!(find_link("").is_none());
        assert!(find_link("just some text").is_none());
        assert!(find_link("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC").is_none());
        assert!(find_link("https://example.com/?list=abc").is_none());
    }

    #[test]
    fn audio_paths_are_recognised_by_extension() {
        assert!(is_audio_path(Path::new("C:\\m\\a.MP3")));
        assert!(is_audio_path(Path::new("b.flac")));
        assert!(!is_audio_path(Path::new("notes.txt")));
        assert!(!is_audio_path(Path::new("noext")));
    }
}
