//! Read-only account data through the supported YouTube Data API.
use crate::ytm::{Account, Item, Layout, Page, Section};
use serde_json::Value;

pub const PLAYLIST_PREFIX: &str = "YT:";
type Query<'a> = &'a [(&'a str, &'a str)];
type Get<'a> = dyn Fn(&str, Query<'_>) -> Result<Value, String> + 'a;

fn string(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn thumbnail(snippet: &Value) -> String {
    ["maxres", "standard", "high", "medium", "default"]
        .into_iter()
        .find_map(|size| snippet["thumbnails"][size]["url"].as_str())
        .unwrap_or_default()
        .to_owned()
}

fn collect(get: &Get<'_>, endpoint: &str, query: Query<'_>) -> Result<Vec<Value>, String> {
    let mut items = Vec::new();
    let mut token = String::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        let mut params = query.to_vec();
        params.push(("maxResults", "50"));
        if !token.is_empty() {
            params.push(("pageToken", &token));
        }
        let response = get(endpoint, &params)?;
        if let Some(page) = response["items"].as_array() {
            items.extend(page.iter().cloned());
        }
        token = string(&response["nextPageToken"]);
        if token.is_empty() {
            return Ok(items);
        }
        if !seen.insert(token.clone()) {
            return Err("YouTube repeated a page token. Try again later.".to_owned());
        }
    }
    Err("This list exceeds the 10,000 item import limit.".to_owned())
}

pub fn account(get: &Get<'_>) -> Result<Account, String> {
    let response = get("channels", &[("part", "snippet"), ("mine", "true")])?;
    let snippet = &response["items"][0]["snippet"];
    Ok(Account {
        id: string(&response["items"][0]["id"]),
        name: snippet["title"].as_str().unwrap_or("YouTube account").to_owned(),
        photo: thumbnail(snippet),
    })
}

fn playlist(value: &Value) -> Item {
    let snippet = &value["snippet"];
    Item {
        title: string(&snippet["title"]),
        subtitle: string(&snippet["channelTitle"]),
        thumb: thumbnail(snippet),
        browse_id: format!("{PLAYLIST_PREFIX}{}", string(&value["id"])),
        extra: value["contentDetails"]["itemCount"]
            .as_u64()
            .map(|n| format!("{n} tracks"))
            .unwrap_or_default(),
        ..Item::default()
    }
}

pub fn library(get: &Get<'_>) -> Result<Vec<Item>, String> {
    Ok(collect(get, "playlists", &[("part", "snippet,contentDetails"), ("mine", "true")])?
        .iter()
        .filter(|v| v["id"].as_str().is_some_and(|id| !id.is_empty()))
        .map(playlist)
        .collect())
}

fn song(value: &Value, in_playlist: bool) -> Option<Item> {
    let snippet = &value["snippet"];
    let id = if in_playlist {
        &snippet["resourceId"]["videoId"]
    } else {
        &value["id"]
    };
    let id = id.as_str().filter(|s| !s.is_empty())?;
    // Removed/private videos cannot be played or meaningfully imported.
    if in_playlist && snippet["videoOwnerChannelTitle"].as_str().is_none() {
        return None;
    }
    let artist = snippet["videoOwnerChannelTitle"]
        .as_str()
        .or_else(|| snippet["channelTitle"].as_str())
        .unwrap_or_default();
    Some(Item {
        title: string(&snippet["title"]),
        subtitle: artist.to_owned(),
        thumb: thumbnail(snippet),
        video_id: id.to_owned(),
        duration: duration(value["contentDetails"]["duration"].as_str().unwrap_or_default()),
        ..Item::default()
    })
}

pub fn liked(get: &Get<'_>) -> Result<Vec<Item>, String> {
    Ok(collect(get, "videos", &[("part", "snippet,contentDetails"), ("myRating", "like")])?
        .iter()
        .filter_map(|v| song(v, false))
        .collect())
}

pub fn browse(get: &Get<'_>, id: &str) -> Result<Page, String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err("Invalid playlist.".to_owned());
    }
    let info = get("playlists", &[("part", "snippet,contentDetails"), ("id", id)])?;
    let header = info["items"]
        .as_array()
        .and_then(|v| v.first())
        .ok_or("This playlist is unavailable to this account.")?;
    let tracks = collect(get, "playlistItems", &[("part", "snippet,contentDetails"), ("playlistId", id)])?;
    Ok(Page {
        header: Some(playlist(header)),
        sections: vec![Section {
            title: "Tracks".to_owned(),
            layout: Layout::Tracks,
            items: tracks.iter().filter_map(|v| song(v, true)).collect(),
        }],
        partial: false,
    })
}

fn duration(iso: &str) -> String {
    let Some(time) = iso.strip_prefix("PT") else {
        return String::new();
    };
    let mut digits = String::new();
    let mut seconds = 0u64;
    for c in time.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let Ok(n) = digits.parse::<u64>() else {
            return String::new();
        };
        digits.clear();
        let factor = match c {
            'H' => 3600,
            'M' => 60,
            'S' => 1,
            _ => return String::new(),
        };
        let Some(total) = n.checked_mul(factor).and_then(|n| seconds.checked_add(n)) else {
            return String::new();
        };
        seconds = total;
    }
    if !digits.is_empty() {
        return String::new();
    }
    if seconds >= 3600 {
        format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn imports_private_playlists_with_official_browse_ids() {
        let get = |endpoint: &str, query: Query<'_>| {
            assert_eq!(endpoint, "playlists");
            assert!(query.contains(&("mine", "true")));
            Ok(if query.contains(&("pageToken", "next")) {
                json!({"items":[{"id":"PLsecond","snippet":{"title":"Second"}}]})
            } else {
                json!({"items":[{"id":"PLprivate","snippet":{"title":"Private","channelTitle":"Me"}}],"nextPageToken":"next"})
            })
        };
        let lists = library(&get).unwrap();
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].browse_id, "YT:PLprivate");
        assert_eq!(lists[1].title, "Second");
    }

    #[test]
    fn playlist_tracks_keep_video_ids_and_skip_deleted_videos() {
        let get = |endpoint: &str, _: Query<'_>| {
            Ok(if endpoint == "playlists" {
                json!({"items":[{"id":"PLx","snippet":{"title":"My music"}}]})
            } else {
                json!({"items":[
                    {"snippet":{"title":"Song","videoOwnerChannelTitle":"Artist","resourceId":{"videoId":"video"}}},
                    {"snippet":{"title":"Deleted video","resourceId":{"videoId":"removed"}}}
                ]})
            })
        };
        let page = browse(&get, "PLx").unwrap();
        assert_eq!(page.songs().len(), 1);
        assert_eq!(page.songs()[0].video_id, "video");
        assert!(browse(&get, "../bad").is_err());
    }

    #[test]
    fn liked_videos_are_paginated_and_have_display_durations() {
        let get = |endpoint: &str, query: Query<'_>| {
            assert_eq!(endpoint, "videos");
            assert!(query.contains(&("myRating", "like")));
            Ok(json!({"items":[{"id":"song","snippet":{"title":"A","channelTitle":"Artist"},"contentDetails":{"duration":"PT1H2M3S"}}]}))
        };
        assert_eq!(liked(&get).unwrap()[0].duration, "1:02:03");
        assert_eq!(duration("PT3M5S"), "3:05");
        assert_eq!(duration("bad"), "");
    }

    #[test]
    fn repeated_page_tokens_fail_instead_of_looping() {
        let get = |_: &str, _: Query<'_>| Ok(json!({"nextPageToken":"same"}));
        assert!(library(&get).unwrap_err().contains("repeated"));
    }
}
