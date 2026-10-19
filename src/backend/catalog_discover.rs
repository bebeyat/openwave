//! Discovery shelves for Home: YouTube Music's own home, explore and charts pages, asked for in
//! the user's language (`hl`) and country (`gl`), without an account. Every shelf keeps the
//! catalog's own (already translated) title, so a user in Japan sees "急上昇" and one in Brazil
//! "Em alta" without a key of ours per row.
use super::{parse::image, search::mrlir_entry, CatalogError, Entry};
use serde_json::{json, Value};
use std::{sync::OnceLock, time::Duration};

const BROWSE_URL: &str = "https://music.youtube.com/youtubei/v1/browse?prettyPrint=false";
const CLIENT_VERSION: &str = "1.20250101.01.00";
/// The charts page's id for "the whole world", used when the country is unknown.
pub const GLOBAL_CHARTS: &str = "ZZ";
const MAX_SHELVES: usize = 12;

/// One row of cards. `kind` is what every item is: `track`, `album`, `list` or `artist`.
#[derive(Clone, Debug)]
pub struct RawShelf {
    pub title: String,
    pub kind: &'static str,
    pub items: Vec<Entry>,
}

/// A mood filter of the home page ("Energize", "Relax"…); `params` asks for its feed.
#[derive(Clone, Debug, PartialEq)]
pub struct RawChip {
    pub label: String,
    pub params: String,
}

#[derive(Clone, Debug, Default)]
pub struct RawDiscover {
    pub chips: Vec<RawChip>,
    pub shelves: Vec<RawShelf>,
}

fn http() -> Result<&'static reqwest::Client, CatalogError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| CatalogError::Network(e.clone()))
}

/// Two-letter language and country codes only: they go into the request body as they are.
fn valid_code(s: &str, len: std::ops::RangeInclusive<usize>) -> bool {
    len.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

async fn browse(hl: &str, gl: Option<&str>, extra: Value) -> Result<Value, CatalogError> {
    let mut client =
        json!({ "clientName": "WEB_REMIX", "clientVersion": CLIENT_VERSION, "hl": hl });
    if let Some(gl) = gl {
        client["gl"] = json!(gl);
    }
    let mut body = json!({ "context": { "client": client } });
    if let (Some(b), Some(e)) = (body.as_object_mut(), extra.as_object()) {
        b.extend(e.clone());
    }
    let response = http()?
        .post(BROWSE_URL)
        // The EU consent page otherwise answers instead of the API.
        .header("Cookie", "SOCS=CAI")
        .header("Origin", "https://music.youtube.com")
        .json(&body)
        .send()
        .await
        .map_err(|e| CatalogError::Network(e.to_string()))?;
    if !response.status().is_success() {
        return Err(CatalogError::BadResponse(format!(
            "HTTP {}",
            response.status()
        )));
    }
    response
        .json()
        .await
        .map_err(|e| CatalogError::BadResponse(e.to_string()))
}

/// Home, explore and charts in parallel, or only the home feed of one mood `chip`. A page that
/// fails is left out; only when all fail is it an error.
pub async fn discover(
    hl: &str,
    gl: Option<&str>,
    chip: Option<&str>,
) -> Result<RawDiscover, CatalogError> {
    if !valid_code(hl, 2..=5) || gl.is_some_and(|g| !valid_code(g, 2..=2)) {
        return Err(CatalogError::InvalidRequest);
    }
    if let Some(params) = chip {
        let home = browse(
            hl,
            gl,
            json!({ "browseId": "FEmusic_home", "params": params }),
        )
        .await?;
        return Ok(RawDiscover {
            chips: chips(&home),
            shelves: shelves(&home),
        });
    }
    let country = gl.unwrap_or(GLOBAL_CHARTS);
    let (home, explore, charts) = futures_util::join!(
        browse(hl, gl, json!({ "browseId": "FEmusic_home" })),
        browse(hl, gl, json!({ "browseId": "FEmusic_explore" })),
        browse(
            hl,
            gl,
            json!({ "browseId": "FEmusic_charts", "formData": { "selectedValues": [country] } })
        ),
    );
    if let (Err(e), Err(_), Err(_)) = (&home, &explore, &charts) {
        return Err(e.clone());
    }
    let page = |r: &Result<Value, CatalogError>, from: Page| {
        r.as_ref()
            .map(shelves)
            .unwrap_or_default()
            .into_iter()
            .map(move |s| (from, s))
    };
    let mut all: Vec<(Page, RawShelf)> = page(&home, Page::Home)
        .chain(page(&explore, Page::Explore))
        .chain(page(&charts, Page::Charts))
        .collect();
    all.sort_by_key(|(from, s)| rank(*from, s.kind));
    let mut all: Vec<RawShelf> = all.into_iter().map(|(_, s)| s).collect();
    all.truncate(MAX_SHELVES);
    Ok(RawDiscover {
        chips: home.as_ref().map(chips).unwrap_or_default(),
        shelves: all,
    })
}

#[derive(Clone, Copy)]
enum Page {
    Home,
    Explore,
    Charts,
}

/// Row order, whatever language the titles are in: quick picks (home songs), new releases,
/// trending songs, the country's charts, top artists, then the rest (community playlists…).
fn rank(from: Page, kind: &str) -> u8 {
    match (from, kind) {
        (Page::Home, "track") => 0,
        (Page::Explore, "album") => 1,
        (Page::Explore, "track") => 2,
        (Page::Charts, "list") => 3,
        (Page::Charts, "artist") => 4,
        _ => 5,
    }
}

fn runs_text(v: &Value) -> String {
    v["runs"]
        .as_array()
        .map(|runs| runs.iter().filter_map(|r| r["text"].as_str()).collect())
        .unwrap_or_default()
}

fn chips(page: &Value) -> Vec<RawChip> {
    let mut out = vec![];
    walk(page, &mut |v| {
        let Some(chips) = v
            .pointer("/chipCloudRenderer/chips")
            .and_then(Value::as_array)
        else {
            return false;
        };
        for c in chips {
            let c = &c["chipCloudChipRenderer"];
            let label = runs_text(&c["text"]);
            let params = c
                .pointer("/navigationEndpoint/browseEndpoint/params")
                .and_then(Value::as_str)
                .unwrap_or("");
            if !label.is_empty() && !params.is_empty() {
                out.push(RawChip {
                    label,
                    params: params.into(),
                });
            }
        }
        true
    });
    out
}

/// Depth-first walk; `visit` returns true when it consumed a node (its children are skipped).
fn walk(v: &Value, visit: &mut impl FnMut(&Value) -> bool) {
    if visit(v) {
        return;
    }
    match v {
        Value::Object(map) => map.values().for_each(|c| walk(c, visit)),
        Value::Array(items) => items.iter().for_each(|c| walk(c, visit)),
        _ => {}
    }
}

/// Every carousel of a page with the cards we can show. A shelf keeps the kind of its first
/// readable item and drops the others (podcasts, mood buttons and the like are skipped).
fn shelves(page: &Value) -> Vec<RawShelf> {
    let mut out = vec![];
    walk(page, &mut |v| {
        let Some(shelf) = v.get("musicCarouselShelfRenderer") else {
            return false;
        };
        let title = runs_text(&shelf["header"]["musicCarouselShelfBasicHeaderRenderer"]["title"]);
        let items: Vec<(&'static str, Entry)> = shelf["contents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(item)
            .collect();
        if let Some(&(kind, _)) = items.first() {
            let items: Vec<Entry> = items
                .into_iter()
                .filter(|(k, _)| *k == kind)
                .map(|(_, e)| e)
                .collect();
            if !title.is_empty() && items.len() >= 3 {
                out.push(RawShelf { title, kind, items });
            }
        }
        true
    });
    out
}

fn page_type(endpoint: &Value) -> &str {
    endpoint
        .pointer("/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn kind_of(page_type: &str) -> Option<&'static str> {
    match page_type {
        "MUSIC_PAGE_TYPE_ALBUM" => Some("album"),
        "MUSIC_PAGE_TYPE_PLAYLIST" => Some("list"),
        "MUSIC_PAGE_TYPE_ARTIST" => Some("artist"),
        _ => None,
    }
}

fn item(card: &Value) -> Option<(&'static str, Entry)> {
    if let Some(row) = card.get("musicResponsiveListItemRenderer") {
        let kind = if row.pointer("/playlistItemData/videoId").is_some() {
            "track"
        } else {
            kind_of(page_type(&row["navigationEndpoint"]))?
        };
        return mrlir_entry(row, kind).map(|e| (kind, e));
    }
    two_row(card.get("musicTwoRowItemRenderer")?)
}

/// Album, playlist, artist or video card (`musicTwoRowItemRenderer`).
fn two_row(card: &Value) -> Option<(&'static str, Entry)> {
    let nav = &card["navigationEndpoint"];
    let (kind, key) = match nav
        .pointer("/watchEndpoint/videoId")
        .and_then(Value::as_str)
    {
        Some(video) => ("track", video),
        None => (
            kind_of(page_type(nav))?,
            nav.pointer("/browseEndpoint/browseId")
                .and_then(Value::as_str)?,
        ),
    };
    let name = runs_text(&card["title"]);
    if name.trim().is_empty() || key.is_empty() {
        return None;
    }
    // The subtitle reads "Single • Artist" or "Artist • 1 M views": the linked runs are artists.
    let runs = card["subtitle"]["runs"].as_array();
    let artists: Vec<(&str, &str)> = runs
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let id = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")?
                .as_str()?;
            Some((r["text"].as_str()?, id)).filter(|_| id.starts_with("UC"))
        })
        .collect();
    let performer = if artists.is_empty() {
        let text = runs_text(&card["subtitle"]);
        text.split(" • ").last().unwrap_or("").trim().to_owned()
    } else {
        artists
            .iter()
            .map(|(n, _)| *n)
            .collect::<Vec<_>>()
            .join(", ")
    };
    Some((
        kind,
        Entry {
            key: key.into(),
            kind: kind.into(),
            name: name.trim().into(),
            performer,
            seconds: 0,
            artwork: card
                .pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail")
                .map(image)
                .unwrap_or_default(),
            album: String::new(),
            artist_key: artists
                .first()
                .map(|(_, id)| (*id).to_owned())
                .unwrap_or_default(),
            album_key: if kind == "album" {
                key.into()
            } else {
                String::new()
            },
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browse_to(id: &str, page: &str) -> Value {
        json!({ "browseEndpoint": { "browseId": id, "browseEndpointContextSupportedConfigs": {
            "browseEndpointContextMusicConfig": { "pageType": page } } } })
    }

    fn album(id: &str, title: &str) -> Value {
        json!({ "musicTwoRowItemRenderer": {
            "title": { "runs": [{ "text": title }] },
            "subtitle": { "runs": [{ "text": "Single" }, { "text": " • " },
                { "text": "Rojda", "navigationEndpoint": browse_to("UCartist", "MUSIC_PAGE_TYPE_ARTIST") }] },
            "navigationEndpoint": browse_to(id, "MUSIC_PAGE_TYPE_ALBUM"),
            "thumbnailRenderer": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://lh3.googleusercontent.com/a=w544-h544", "width": 544, "height": 544 }] } } }
        } })
    }

    fn song(id: &str, title: &str) -> Value {
        json!({ "musicResponsiveListItemRenderer": {
            "playlistItemData": { "videoId": id },
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": title }] } } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                    { "text": "Blok3", "navigationEndpoint": browse_to("UCblok3", "MUSIC_PAGE_TYPE_ARTIST") }] } } }
            ]
        } })
    }

    fn podcast() -> Value {
        json!({ "musicResponsiveListItemRenderer": {
            "navigationEndpoint": browse_to("MPSPx", "MUSIC_PAGE_TYPE_PODCAST_SHOW_DETAIL_PAGE"),
            "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Show" }] } } }]
        } })
    }

    fn shelf(title: &str, contents: Vec<Value>) -> Value {
        json!({ "musicCarouselShelfRenderer": {
            "header": { "musicCarouselShelfBasicHeaderRenderer": { "title": { "runs": [{ "text": title }] } } },
            "contents": contents
        } })
    }

    #[test]
    fn reads_titled_shelves_of_one_kind_and_skips_the_rest() {
        let page = json!({ "contents": [
            shelf("Hızlı seçimler", vec![song("aaaaaaaaaaa", "Bir"), song("bbbbbbbbbbb", "İki"), song("ccccccccccc", "Üç")]),
            shelf("Yeni albümler", vec![album("MPREb_1", "A"), song("ddddddddddd", "x"), album("MPREb_2", "B"), album("MPREb_3", "C")]),
            shelf("Podcasts", vec![podcast(), podcast(), podcast()]),
            shelf("Too short", vec![album("MPREb_4", "D")]),
        ] });
        let s = shelves(&page);
        assert_eq!(s.len(), 2);
        assert_eq!(
            (s[0].title.as_str(), s[0].kind, s[0].items.len()),
            ("Hızlı seçimler", "track", 3)
        );
        assert_eq!(s[0].items[0].performer, "Blok3");
        assert_eq!((s[1].kind, s[1].items.len()), ("album", 3));
        let a = &s[1].items[0];
        assert_eq!(
            (a.key.as_str(), a.performer.as_str(), a.artist_key.as_str()),
            ("MPREb_1", "Rojda", "UCartist")
        );
        assert!(!a.artwork.is_empty());
    }

    #[test]
    fn chips_need_a_label_and_params() {
        let page = json!({ "header": { "chipCloudRenderer": { "chips": [
            { "chipCloudChipRenderer": { "text": { "runs": [{ "text": "Enerjik" }] },
                "navigationEndpoint": { "browseEndpoint": { "browseId": "FEmusic_home", "params": "ggM8" } } } },
            { "chipCloudChipRenderer": { "text": { "runs": [{ "text": "Broken" }] } } }
        ] } } });
        assert_eq!(
            chips(&page),
            vec![RawChip {
                label: "Enerjik".into(),
                params: "ggM8".into()
            }]
        );
    }

    #[test]
    fn quick_picks_lead_and_unknown_rows_trail() {
        assert!(rank(Page::Home, "track") < rank(Page::Explore, "album"));
        assert!(rank(Page::Charts, "artist") < rank(Page::Home, "list"));
        assert_eq!(rank(Page::Explore, "artist"), rank(Page::Home, "list"));
    }

    #[test]
    fn codes_are_checked_before_any_request() {
        assert!(valid_code("tr", 2..=5) && valid_code("pt-BR", 2..=5) && valid_code("TR", 2..=2));
        assert!(!valid_code("t\"r", 2..=5) && !valid_code("TUR", 2..=2) && !valid_code("", 2..=5));
    }
}
