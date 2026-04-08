use anyhow::{anyhow, Result};
use reqwest::header::{ACCEPT, CONTENT_TYPE, ORIGIN, REFERER};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const BASE_URL: &str = "https://howlongtobeat.com/";
const API_FINDER_INIT: &str = "https://howlongtobeat.com/api/finder/init";
const API_FINDER: &str = "https://howlongtobeat.com/api/finder";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HltbEnrich {
    pub hltb_id: i64,
    pub title: String,
    pub cover_url: Option<String>,
    pub profile_url: String,
    pub release_year: Option<i32>,
    pub platforms: Option<String>,
    pub genres: Option<String>,
    pub summary: Option<String>,
    pub hltb_main: Option<i64>,
    pub hltb_extra: Option<i64>,
    pub hltb_complete: Option<i64>,
    pub hltb_all: Option<i64>,
    pub hltb_main_count: Option<i64>,
    pub hltb_extra_count: Option<i64>,
    pub hltb_complete_count: Option<i64>,
    pub hltb_all_count: Option<i64>,
    pub match_method: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FinderInit {
    token: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FinderResponse {
    data: Vec<FinderGame>,
}

#[derive(Debug, Clone, Deserialize)]
struct FinderGame {
    game_id: i64,
    game_name: String,
    #[serde(default)]
    game_alias: String,
    #[serde(default)]
    game_type: String,
    game_image: Option<String>,
    comp_main: Option<i64>,
    comp_plus: Option<i64>,
    comp_100: Option<i64>,
    comp_all: Option<i64>,
    comp_main_count: Option<i64>,
    comp_plus_count: Option<i64>,
    comp_100_count: Option<i64>,
    comp_all_count: Option<i64>,
    profile_platform: Option<String>,
    release_world: Option<Value>,
}

#[derive(Debug, Clone)]
struct RankedFinderGame {
    game: FinderGame,
    score: i32,
    exact_match: bool,
    alias_match: bool,
}

fn hltb_client() -> Result<Client> {
    Client::builder()
        .cookie_store(true)
        .user_agent(USER_AGENT)
        .build()
        .map_err(Into::into)
}

async fn prime_session(client: &Client) -> Result<()> {
    let resp = client
        .get(BASE_URL)
        .header(
            ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header(REFERER, BASE_URL)
        .header(ORIGIN, "https://howlongtobeat.com")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("HLTB homepage error: {}", resp.status()));
    }
    Ok(())
}

async fn finder_token(client: &Client) -> Result<String> {
    prime_session(client).await?;

    let resp = client
        .get(format!(
            "{}?t={}",
            API_FINDER_INIT,
            chrono::Utc::now().timestamp_millis()
        ))
        .header(ACCEPT, "application/json")
        .header(REFERER, BASE_URL)
        .header(ORIGIN, "https://howlongtobeat.com")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("HLTB finder init error: {}", resp.status()));
    }

    let init: FinderInit = resp.json().await?;
    if let Some(token) = init.token.filter(|t| !t.trim().is_empty()) {
        return Ok(token);
    }

    Err(anyhow!(
        "HLTB finder init denied: {}",
        init.error.unwrap_or_else(|| "missing token".to_string())
    ))
}

fn strip_trademark_symbols(input: &str) -> String {
    input
        .chars()
        .filter(|c| !matches!(c, '\u{2122}' | '\u{00AE}' | '\u{00A9}' | '\u{2120}'))
        .collect()
}

fn normalize_title(input: &str) -> String {
    let stripped = strip_trademark_symbols(input)
        .replace('&', " and ")
        .replace('\u{0394}', " delta ")
        .replace('/', " ")
        .replace(':', " ")
        .replace('-', " ");

    stripped
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_trailing_year_group(input: &str) -> String {
    let s = input.trim();
    if let Some(open_idx) = s.rfind(" (") {
        if s.ends_with(')') {
            let inner = &s[open_idx + 2..s.len().saturating_sub(1)];
            let yearish = inner
                .chars()
                .all(|c| c.is_ascii_digit() || c == ' ' || c == '-');
            if yearish && inner.chars().any(|c| c.is_ascii_digit()) {
                return s[..open_idx].trim().to_string();
            }
        }
    }
    s.to_string()
}

fn strip_known_suffixes(input: &str) -> String {
    let mut out = input.trim().to_string();
    let suffixes = [
        " game of the year edition",
        " complete edition",
        " definitive edition",
        " ultimate edition",
        " deluxe edition",
        " remastered",
        " edition",
    ];

    loop {
        let lower = out.to_ascii_lowercase();
        let mut changed = false;
        for suffix in suffixes {
            if lower.ends_with(suffix) {
                out = out[..out.len().saturating_sub(suffix.len())]
                    .trim_end_matches(|c: char| c == ' ' || c == ':' || c == '-')
                    .trim()
                    .to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }

    out
}

fn build_search_variants(input: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let base = input.trim();
    for candidate in [
        base.to_string(),
        strip_trailing_year_group(base),
        strip_known_suffixes(base),
    ] {
        let candidate = candidate.trim();
        if !candidate.is_empty()
            && !out
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(candidate))
        {
            out.push(candidate.to_string());
        }
    }

    if let Some((head, _)) = base.split_once(':') {
        let head = head.trim();
        if !head.is_empty()
            && !out
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(head))
        {
            out.push(head.to_string());
        }
    }

    if let Some((head, _)) = base.split_once(" - ") {
        let head = head.trim();
        if !head.is_empty()
            && !out
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(head))
        {
            out.push(head.to_string());
        }
    }

    out
}

fn title_tokens(input: &str) -> Vec<String> {
    normalize_title(input)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn platform_aliases(platform: Option<&str>) -> Vec<&'static str> {
    let Some(platform) = platform.map(str::trim).filter(|s| !s.is_empty()) else {
        return Vec::new();
    };

    let lower = platform.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "steam" | "gog" | "epic" | "amazon" | "pc" | "windows"
    ) {
        return vec!["pc"];
    }
    if lower.contains("xbox") {
        return vec!["xbox", "xbox one", "xbox series"];
    }
    if lower.contains("playstation") || lower == "ps4" || lower == "ps5" {
        return vec!["playstation", "playstation 4", "playstation 5"];
    }
    if lower.contains("switch") || lower.contains("nintendo") {
        return vec!["nintendo", "switch"];
    }
    if lower.contains("luna") {
        return vec!["amazon luna", "pc"];
    }

    Vec::new()
}

fn parse_aliases(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|alias| !alias.is_empty())
        .map(str::to_string)
        .collect()
}

fn score_candidate(
    candidate: &FinderGame,
    query: &str,
    platform: Option<&str>,
) -> RankedFinderGame {
    let normalized_query = normalize_title(query);
    let query_tokens = title_tokens(query);
    let normalized_name = normalize_title(&candidate.game_name);
    let aliases = parse_aliases(&candidate.game_alias);
    let normalized_aliases = aliases
        .iter()
        .map(|alias| normalize_title(alias))
        .collect::<Vec<_>>();

    let exact_match = normalized_name == normalized_query;
    let alias_match = normalized_aliases
        .iter()
        .any(|alias| alias == &normalized_query);

    let mut score = 0;
    if exact_match {
        score += 120;
    } else if alias_match {
        score += 115;
    } else if normalized_name.starts_with(&normalized_query) {
        score += 90;
    } else if normalized_name.contains(&normalized_query) {
        score += 70;
    }

    for alias in &normalized_aliases {
        if alias.starts_with(&normalized_query) {
            score += 75;
            break;
        }
        if alias.contains(&normalized_query) {
            score += 55;
            break;
        }
    }

    let candidate_tokens = title_tokens(&candidate.game_name);
    let overlap = query_tokens
        .iter()
        .filter(|token| candidate_tokens.iter().any(|cand| cand == *token))
        .count() as i32;
    score += overlap * 10;

    if !query_tokens.is_empty() && overlap == query_tokens.len() as i32 {
        score += 20;
    }

    if candidate.game_type != "game" {
        score -= 20;
    }

    if candidate.comp_main.unwrap_or_default() > 0 {
        score += 5;
    }

    if let Some(candidate_platforms) = candidate.profile_platform.as_deref() {
        let lower = candidate_platforms.to_ascii_lowercase();
        let aliases = platform_aliases(platform);
        if !aliases.is_empty() {
            if aliases.iter().any(|alias| lower.contains(alias)) {
                score += 15;
            } else {
                score -= 10;
            }
        }
    }

    RankedFinderGame {
        game: candidate.clone(),
        score,
        exact_match,
        alias_match,
    }
}

fn is_confident_match(best: &RankedFinderGame, second: Option<&RankedFinderGame>) -> bool {
    if best.exact_match || best.alias_match {
        return true;
    }

    let gap = second
        .map(|candidate| best.score - candidate.score)
        .unwrap_or(best.score);
    best.score >= 95 || (best.score >= 80 && gap >= 20)
}

fn parse_release_year(value: Option<&Value>) -> Option<i32> {
    match value {
        Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
        Some(Value::String(s)) => s
            .chars()
            .collect::<String>()
            .get(0..4)
            .and_then(|year| year.parse::<i32>().ok()),
        _ => None,
    }
}

fn cover_url_from_image(game_image: Option<&str>) -> Option<String> {
    let image = game_image?.trim();
    if image.is_empty() {
        return None;
    }
    Some(format!(
        "https://howlongtobeat.com/games/{}?width=250",
        image
    ))
}

fn profile_url(game_id: i64) -> String {
    format!("https://howlongtobeat.com/game/{}", game_id)
}

fn finder_game_to_enrich(candidate: FinderGame, match_method: &'static str) -> HltbEnrich {
    HltbEnrich {
        hltb_id: candidate.game_id,
        title: candidate.game_name,
        cover_url: cover_url_from_image(candidate.game_image.as_deref()),
        profile_url: profile_url(candidate.game_id),
        release_year: parse_release_year(candidate.release_world.as_ref()),
        platforms: candidate.profile_platform,
        genres: None,
        summary: None,
        hltb_main: candidate.comp_main,
        hltb_extra: candidate.comp_plus,
        hltb_complete: candidate.comp_100,
        hltb_all: candidate.comp_all,
        hltb_main_count: candidate.comp_main_count,
        hltb_extra_count: candidate.comp_plus_count,
        hltb_complete_count: candidate.comp_100_count,
        hltb_all_count: candidate.comp_all_count,
        match_method: Some(match_method.to_string()),
    }
}

async fn finder_search(client: &Client, title: &str) -> Result<Vec<FinderGame>> {
    let token = finder_token(client).await?;
    let terms = title
        .split_whitespace()
        .filter(|term| !term.trim().is_empty())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let body = json!({
        "searchType": "games",
        "searchTerms": terms,
        "searchPage": 1,
        "size": 20,
        "searchOptions": {
            "games": {
                "userId": 0,
                "platform": "",
                "sortCategory": "popular",
                "rangeCategory": "main",
                "rangeTime": { "min": 0, "max": 0 },
                "gameplay": {
                    "perspective": "",
                    "flow": "",
                    "genre": "",
                    "difficulty": ""
                },
                "rangeYear": { "min": "", "max": "" },
                "modifier": ""
            },
            "users": { "sortCategory": "postcount" },
            "lists": { "sortCategory": "follows" },
            "filter": "",
            "sort": 0,
            "randomizer": 0
        },
        "useCache": true
    });

    let resp = client
        .post(API_FINDER)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .header(REFERER, BASE_URL)
        .header(ORIGIN, "https://howlongtobeat.com")
        .header("x-auth-token", token)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "HLTB finder error: {} {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }

    let body: FinderResponse = resp.json().await?;
    Ok(body.data)
}

pub async fn lookup_best_for_title(
    title: &str,
    platform: Option<&str>,
) -> Result<Option<HltbEnrich>> {
    let client = hltb_client()?;
    let mut best_seen: Option<RankedFinderGame> = None;
    let mut second_seen: Option<RankedFinderGame> = None;

    for query in build_search_variants(title) {
        let mut ranked = finder_search(&client, &query)
            .await?
            .into_iter()
            .map(|candidate| score_candidate(&candidate, title, platform))
            .collect::<Vec<_>>();

        ranked.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.game.game_name.cmp(&right.game.game_name))
        });

        if ranked.is_empty() {
            continue;
        }

        let best = ranked[0].clone();
        let second = ranked.get(1).cloned();

        if best_seen
            .as_ref()
            .map(|current| best.score > current.score)
            .unwrap_or(true)
        {
            best_seen = Some(best.clone());
            second_seen = second.clone();
        }

        if is_confident_match(&best, second.as_ref()) {
            let method = if best.alias_match {
                "auto-alias"
            } else {
                "auto-title"
            };
            return Ok(Some(finder_game_to_enrich(best.game, method)));
        }
    }

    let Some(best) = best_seen else {
        return Ok(None);
    };
    if is_confident_match(&best, second_seen.as_ref()) {
        let method = if best.alias_match {
            "auto-alias"
        } else {
            "auto-title"
        };
        return Ok(Some(finder_game_to_enrich(best.game, method)));
    }

    Ok(None)
}

fn value_at<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    value.pointer(pointer)
}

fn string_at(value: &Value, pointer: &str) -> Option<String> {
    value_at(value, pointer)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn i64_at(value: &Value, pointer: &str) -> Option<i64> {
    value_at(value, pointer).and_then(Value::as_i64)
}

fn extract_next_data(html: &str) -> Result<&str> {
    let marker = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let start = html
        .find(marker)
        .ok_or_else(|| anyhow!("HLTB page did not include __NEXT_DATA__"))?;
    let json_start = start + marker.len();
    let rest = &html[json_start..];
    let end = rest
        .find("</script>")
        .ok_or_else(|| anyhow!("HLTB page JSON block was incomplete"))?;
    Ok(&rest[..end])
}

pub async fn lookup_by_id(hltb_id: i64) -> Result<Option<HltbEnrich>> {
    let client = hltb_client()?;
    let resp = client
        .get(profile_url(hltb_id))
        .header(
            ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header(REFERER, BASE_URL)
        .header(ORIGIN, "https://howlongtobeat.com")
        .send()
        .await?;

    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(anyhow!("HLTB game page error: {}", resp.status()));
    }

    let html = resp.text().await?;
    let raw_json = extract_next_data(&html)?;
    let data: Value = serde_json::from_str(raw_json)?;
    let game = value_at(&data, "/props/pageProps/game/data/game/0")
        .ok_or_else(|| anyhow!("HLTB page did not include a game payload"))?;

    let found_id = i64_at(game, "/game_id").unwrap_or(hltb_id);
    let title = string_at(game, "/game_name")
        .ok_or_else(|| anyhow!("HLTB page did not include a game title"))?;

    Ok(Some(HltbEnrich {
        hltb_id: found_id,
        title,
        cover_url: string_at(game, "/game_image")
            .as_deref()
            .and_then(|image| cover_url_from_image(Some(image))),
        profile_url: profile_url(found_id),
        release_year: value_at(game, "/release_world")
            .and_then(|value| parse_release_year(Some(value))),
        platforms: string_at(game, "/profile_platform"),
        genres: string_at(game, "/profile_genre"),
        summary: string_at(game, "/profile_summary"),
        hltb_main: i64_at(game, "/comp_main"),
        hltb_extra: i64_at(game, "/comp_plus"),
        hltb_complete: i64_at(game, "/comp_100"),
        hltb_all: i64_at(game, "/comp_all"),
        hltb_main_count: i64_at(game, "/comp_main_count"),
        hltb_extra_count: i64_at(game, "/comp_plus_count"),
        hltb_complete_count: i64_at(game, "/comp_100_count"),
        hltb_all_count: i64_at(game, "/comp_all_count"),
        match_method: Some("manual-id".to_string()),
    }))
}
