use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct VanityRespOuter {
    response: VanityResp,
}
#[derive(Deserialize, Debug)]
struct VanityResp {
    success: i32,
    steamid: Option<String>,
    message: Option<String>,
}

/// If input is already a 64-bit steamid, return as-is. Otherwise, resolve vanity via Steam Web API.
pub async fn resolve_to_steamid(api_key: &str, input: &str) -> Result<String> {
    // If it's already a 64-bit id, return it
    let trimmed = input.trim();
    if trimmed.chars().all(|c| c.is_ascii_digit()) && trimmed.len() >= 17 {
        return Ok(trimmed.to_string());
    }

    // Extract vanity from full profile URL if provided
    let vanity = extract_profile_tail(trimmed);

    // Build URL with proper query encoding
    let mut url =
        reqwest::Url::parse("https://api.steampowered.com/ISteamUser/ResolveVanityURL/v1/")?;
    url.query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("vanityurl", vanity);

    let resp: VanityRespOuter = reqwest::get(url).await?.json().await?;
    if resp.response.success == 1 {
        Ok(resp.response.steamid.unwrap())
    } else {
        Err(anyhow!(resp.response.message.unwrap_or_else(|| {
            "Failed to resolve vanity URL".to_string()
        })))
    }
}

/// Extract the last path segment from an input that might be a full Steam profile URL.
fn extract_profile_tail(input: &str) -> &str {
    input
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(input)
}

fn build_community_games_xml_url(profile: &str) -> String {
    let trimmed = profile.trim();
    if trimmed.chars().all(|c| c.is_ascii_digit()) && trimmed.len() >= 17 {
        return format!(
            "https://steamcommunity.com/profiles/{}/games/?tab=all&xml=1",
            trimmed
        );
    }

    if let Ok(url) = reqwest::Url::parse(trimmed) {
        let is_steamcommunity = url
            .host_str()
            .map(|host| host.eq_ignore_ascii_case("steamcommunity.com"))
            .unwrap_or(false);
        if is_steamcommunity {
            let mut parts: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
            while matches!(parts.last(), Some(segment) if segment.is_empty()) {
                parts.pop();
            }
            if matches!(parts.last(), Some(segment) if segment.eq_ignore_ascii_case("games")) {
                parts.pop();
            }
            if parts.len() >= 2
                && (parts[0].eq_ignore_ascii_case("profiles")
                    || parts[0].eq_ignore_ascii_case("id"))
            {
                return format!(
                    "https://steamcommunity.com/{}/{}/games/?tab=all&xml=1",
                    parts[0], parts[1]
                );
            }
        }
    }

    let vanity = extract_profile_tail(trimmed);
    format!(
        "https://steamcommunity.com/id/{}/games/?tab=all&xml=1",
        vanity
    )
}

#[derive(Deserialize, Debug)]
struct OwnedGamesOuter {
    response: OwnedGames,
}
#[derive(Deserialize, Debug)]
struct OwnedGames {
    games: Option<Vec<SteamGame>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SteamGame {
    pub appid: i32,
    pub name: String,
    pub playtime_forever: Option<i32>,  // minutes
    pub rtime_last_played: Option<i64>, // unix seconds
}

/// Official Web API (needs API key + steamid)
pub async fn fetch_owned_games(api_key: &str, steam_id: &str) -> Result<Vec<SteamGame>> {
    let mut url =
        reqwest::Url::parse("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/")?;
    url.query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("steamid", steam_id)
        .append_pair("include_appinfo", "1")
        .append_pair("include_played_free_games", "1")
        .append_pair("include_free_sub", "1");

    let outer: OwnedGamesOuter = reqwest::get(url).await?.json().await?;
    Ok(outer.response.games.unwrap_or_default())
}

/// Community XML fallback (no API key). Requires the user's profile and game details to be Public.
/// Accepts either a vanity, steamid64, or full profile URL.
pub async fn fetch_owned_games_via_xml(profile: &str) -> Result<Vec<SteamGame>> {
    let url = build_community_games_xml_url(profile);

    println!("[steam.xml] URL = {}", url);

    let xml = reqwest::get(&url).await?.text().await?;
    println!(
        "[steam.xml] First 200 chars = {}",
        &xml.chars().take(200).collect::<String>()
    );
    let xml_lower = xml.to_ascii_lowercase();
    if !xml_lower.contains("<gameslist") && !xml_lower.contains("<game>") {
        return Err(anyhow!(
            "Steam Community XML did not return a visible games list. Without an API key, the profile and game details must be public, and the XML fallback can still be unreliable."
        ));
    }

    // --- Path A: quick-xml reader (best-effort) ---
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(&xml);
    reader.trim_text(true);

    let mut buf = Vec::new();

    #[derive(Default)]
    struct Temp {
        appid: Option<i32>,
        name: Option<String>,
        hours_on_record: Option<String>,
    }

    let mut current = Temp::default();
    let mut in_game = false;
    let mut in_tag: Option<String> = None;
    let mut out: Vec<SteamGame> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag.eq_ignore_ascii_case("game") {
                    in_game = true;
                    current = Temp::default();
                } else if in_game {
                    in_tag = Some(tag);
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag.eq_ignore_ascii_case("game") && in_game {
                    if let (Some(appid), Some(name)) = (current.appid, current.name.take()) {
                        let minutes = parse_hours_to_minutes(current.hours_on_record.as_deref());
                        out.push(SteamGame {
                            appid,
                            name,
                            playtime_forever: Some(minutes),
                            rtime_last_played: None,
                        });
                    }
                    in_game = false;
                    in_tag = None;
                } else if in_game {
                    in_tag = None;
                }
            }
            Ok(Event::Text(t)) => {
                if in_game {
                    if let Some(ref tag) = in_tag {
                        let txt = t.unescape().unwrap_or_default().to_string();
                        match tag.as_str() {
                            "appID" => {
                                if let Ok(v) = txt.parse::<i32>() {
                                    current.appid = Some(v);
                                }
                            }
                            "name" => current.name = Some(txt),
                            "hoursOnRecord" => current.hours_on_record = Some(txt),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                println!("[steam.xml] quick-xml error: {e}");
                break; // we'll fall back below
            }
            _ => {}
        }
        buf.clear();
    }

    println!("[steam.xml] Parsed (quick-xml) = {}", out.len());

    // --- Path B: string-scan fallback if quick-xml found nothing ---
    if out.is_empty() {
        let mut fallback: Vec<SteamGame> = Vec::new();

        // split by <game>...</game>
        let mut start = 0usize;
        while let Some(s) = xml[start..].find("<game>") {
            let sidx = start + s;
            let eidx = if let Some(e) = xml[sidx..].find("</game>") {
                sidx + e + "</game>".len()
            } else {
                break;
            };
            let chunk = &xml[sidx..eidx];

            // extract helpers
            fn between<'a>(hay: &'a str, open: &str, close: &str) -> Option<&'a str> {
                let a = hay.find(open)? + open.len();
                let b = hay[a..].find(close)? + a;
                Some(&hay[a..b])
            }

            let appid =
                between(chunk, "<appID>", "</appID>").and_then(|s| s.trim().parse::<i32>().ok());
            let name = between(chunk, "<name>", "</name>").map(|s| {
                // strip CDATA if present
                let t = s.trim();
                if let Some(inner) = t
                    .strip_prefix("<![CDATA[")
                    .and_then(|u| u.strip_suffix("]]>"))
                {
                    inner.to_string()
                } else {
                    t.to_string()
                }
            });
            let hours =
                between(chunk, "<hoursOnRecord>", "</hoursOnRecord>").map(|s| s.to_string());

            if let (Some(appid), Some(name)) = (appid, name) {
                let minutes = parse_hours_to_minutes(hours.as_deref());
                fallback.push(SteamGame {
                    appid,
                    name,
                    playtime_forever: Some(minutes),
                    rtime_last_played: None,
                });
            }

            start = eidx; // advance
        }

        println!("[steam.xml] Parsed (fallback) = {}", fallback.len());
        if !fallback.is_empty() {
            return Ok(fallback);
        }
    }

    Ok(out)
}

fn parse_hours_to_minutes(h: Option<&str>) -> i32 {
    if let Some(s) = h {
        let digits: String = s
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if let Ok(val) = digits.parse::<f32>() {
            return (val * 60.0) as i32;
        }
    }
    0
}

pub fn steam_header_image(appid: i32) -> String {
    format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg",
        appid
    )
}

pub fn unix_to_iso(ts: i64) -> String {
    if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0) {
        dt.to_rfc3339()
    } else {
        chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0)
            .unwrap()
            .to_rfc3339()
    }
}
