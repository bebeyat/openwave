//! Genre browsing: the catalog's genre list and the playlists of one genre, flattened to
//! `{ "items": [{ "name", "key" }] }` like the other sections.

use serde_json::{json, Value};
use ytmapi_rs::parse::MoodCategorySection;

/// Genres from the categories page: the section named like "Genres" (the other one is moods);
/// when no name matches (another UI language), the last section, which is where genres sit.
pub(super) fn genre_items(sections: &[MoodCategorySection]) -> Value {
    let named = sections.iter().find(|s| {
        let n = s.section_name.to_lowercase();
        n.contains("genre") || n.contains("tür")
    });
    let items: Vec<Value> = named
        .or(sections.last())
        .map(|s| {
            s.mood_categories
                .iter()
                .filter_map(|c| {
                    let key = serde_json::to_value(&c.params).ok()?;
                    Some(json!({ "name": c.title, "key": key.as_str()? }))
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "items": items })
}

/// Playlists of one genre page, in page order: every two-row card that opens a playlist
/// (`VL…` browse id). The typed parser of the client library fails on this page, so the raw
/// response is walked instead.
pub(super) fn genre_playlists(raw: &Value) -> Value {
    let mut items = vec![];
    collect_cards(raw, &mut items);
    let mut seen = std::collections::HashSet::new();
    items.retain(|v: &Value| seen.insert(v["key"].as_str().unwrap_or("").to_owned()));
    json!({ "items": items })
}

fn collect_cards(v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::Object(map) => {
            if let Some(card) = map.get("musicTwoRowItemRenderer") {
                let name = card.pointer("/title/runs/0/text").and_then(Value::as_str);
                let key = card
                    .pointer("/navigationEndpoint/browseEndpoint/browseId")
                    .and_then(Value::as_str);
                if let (Some(name), Some(key)) = (name, key) {
                    if key.starts_with("VL") {
                        out.push(json!({ "name": name, "key": key }));
                    }
                }
            }
            map.values().for_each(|x| collect_cards(x, out));
        }
        Value::Array(list) => list.iter().for_each(|x| collect_cards(x, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_playlist_cards_in_order_without_repeats() {
        let card = |t: &str, id: &str| {
            json!({"musicTwoRowItemRenderer": {
                "title": {"runs": [{"text": t}]},
                "navigationEndpoint": {"browseEndpoint": {"browseId": id}}
            }})
        };
        let raw = json!({"contents": [
            {"shelf": {"contents": [card("Hits", "VLRDA"), card("Artist", "UCx")]}},
            {"shelf": {"contents": [card("Party", "VLRDB"), card("Hits again", "VLRDA")]}}
        ]});
        let items = genre_playlists(&raw)["items"].clone();
        assert_eq!(
            items,
            json!([{"name": "Hits", "key": "VLRDA"}, {"name": "Party", "key": "VLRDB"}])
        );
        assert_eq!(genre_playlists(&json!({}))["items"], json!([]));
    }

    #[test]
    fn prefers_the_genres_section() {
        let section = |name: &str, titles: &[&str]| -> MoodCategorySection {
            serde_json::from_value(json!({
                "section_name": name,
                "mood_categories": titles
                    .iter()
                    .map(|t| json!({"title": t, "params": format!("p{t}")}))
                    .collect::<Vec<_>>()
            }))
            .unwrap()
        };
        let both = [
            section("Genres", &["Pop"]),
            section("Moods & moments", &["Chill"]),
        ];
        assert_eq!(
            genre_items(&both)["items"],
            json!([{"name": "Pop", "key": "pPop"}])
        );
        let unnamed = [section("A", &["Chill"]), section("B", &["Rock"])];
        assert_eq!(genre_items(&unnamed)["items"][0]["name"], "Rock");
        assert_eq!(genre_items(&[])["items"], json!([]));
    }
}
