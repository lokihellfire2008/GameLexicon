use anyhow::{anyhow, Result};
use chrono::{Datelike, TimeZone, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize}; // add Serialize here

fn strip_trademark_symbols(input: &str) -> String {
    input
        .chars()
        .filter(|c| !matches!(c, '\u{2122}' | '\u{00AE}' | '\u{00A9}' | '\u{2120}'))
        .collect()
}

fn split_compound_words(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len() + 8);

    for i in 0..chars.len() {
        let ch = chars[i];
        let prev = if i > 0 { Some(chars[i - 1]) } else { None };
        let next = if i + 1 < chars.len() {
            Some(chars[i + 1])
        } else {
            None
        };

        let boundary_before_upper = ch.is_ascii_uppercase()
            && (prev
                .map(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
                .unwrap_or(false)
                || (prev.map(|p| p.is_ascii_uppercase()).unwrap_or(false)
                    && next.map(|n| n.is_ascii_lowercase()).unwrap_or(false)));
        let boundary_before_digit =
            ch.is_ascii_digit() && prev.map(|p| p.is_ascii_alphabetic()).unwrap_or(false);
        let boundary_after_digit =
            ch.is_ascii_alphabetic() && prev.map(|p| p.is_ascii_digit()).unwrap_or(false);

        let needs_space =
            i > 0 && (boundary_before_upper || boundary_before_digit || boundary_after_digit);
        let last_is_space = out.as_bytes().last().copied() == Some(b' ');
        if needs_space && !last_is_space {
            out.push(' ');
        }
        out.push(ch);
    }

    out
}

fn normalize_title_for_igdb_search(input: &str) -> String {
    let leading_clean = input
        .trim_start()
        .trim_start_matches(|c: char| {
            matches!(
                c,
                '+' | '*' | '\u{2022}' | '\u{00B7}' | '\u{25CF}' | '\u{25BA}' | '\u{25B6}'
            )
        })
        .trim_start();
    let stripped = strip_trademark_symbols(leading_clean);
    let split = split_compound_words(&stripped);
    let squashed = split.split_whitespace().collect::<Vec<_>>().join(" ");
    if squashed.is_empty() {
        input.trim().to_string()
    } else {
        squashed
    }
}

fn expand_common_acronyms(input: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for raw in input.split_whitespace() {
        let token = raw
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .to_ascii_lowercase();
        if token == "goty" {
            out.push("game".to_string());
            out.push("of".to_string());
            out.push("the".to_string());
            out.push("year".to_string());
        } else if token == "tc" || token == "tcs" || token == "tc's" {
            out.push("tom".to_string());
            out.push("clancy's".to_string());
        } else if token == "r6" {
            out.push("rainbow".to_string());
            out.push("six".to_string());
        } else {
            out.push(raw.to_string());
        }
    }
    out.join(" ")
}

fn title_case_ascii(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let mut s = String::new();
                    s.push_str(&first.to_uppercase().collect::<String>());
                    s.push_str(&chars.as_str().to_ascii_lowercase());
                    s
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn case_variants(input: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for candidate in [
        input.trim().to_string(),
        input.trim().to_ascii_lowercase(),
        title_case_ascii(input.trim()),
    ] {
        let normalized = candidate.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

fn push_unique_variant(out: &mut Vec<String>, candidate: String) {
    let normalized = normalize_title_for_igdb_search(&candidate);
    if normalized.is_empty() {
        return;
    }
    if out.iter().any(|v| v.eq_ignore_ascii_case(&normalized)) {
        return;
    }
    out.push(normalized);
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

fn strip_known_title_suffixes(input: &str) -> String {
    let mut out = input.trim().to_string();
    let suffixes = [
        " game of the year edition",
        " complete edition",
        " definitive edition",
        " ultimate edition",
        " deluxe edition",
        " directors cut",
        " director's cut",
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

fn escape_apicalypse_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\"', "'")
}

fn slugify_for_igdb(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in normalize_title_for_igdb_search(input).chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn token_fallbacks(input: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let cleaned = normalize_title_for_igdb_search(input);
    for token in cleaned
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
    {
        let t = token.to_ascii_lowercase();
        if !out.iter().any(|x| x == &t) {
            out.push(t);
        }
    }
    out
}

fn push_unique_phrase(out: &mut Vec<String>, phrase: String) {
    let normalized = phrase.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() < 5 {
        return;
    }
    if !out.iter().any(|v| v == &normalized) {
        out.push(normalized);
    }
}

fn phrase_fallbacks(input: &str) -> Vec<String> {
    let cleaned = normalize_title_for_igdb_search(input);
    let tokens: Vec<String> = cleaned
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(|t| {
            let s = t.trim().to_ascii_lowercase();
            if s.is_empty() || s == "s" {
                None
            } else {
                Some(s)
            }
        })
        .collect();

    if tokens.len() < 2 {
        return Vec::new();
    }

    let mut out = Vec::<String>::new();
    push_unique_phrase(&mut out, tokens.join(" "));
    if tokens.len() >= 3 {
        push_unique_phrase(&mut out, tokens[1..].join(" "));
    }
    if tokens.len() >= 4 {
        push_unique_phrase(&mut out, tokens[2..].join(" "));
    }

    for window in (2..=4).rev() {
        if tokens.len() < window {
            continue;
        }
        for start in 0..=tokens.len() - window {
            let slice = &tokens[start..start + window];
            if slice.iter().any(|t| t.len() >= 4) {
                push_unique_phrase(&mut out, slice.join(" "));
            }
            if out.len() >= 10 {
                return out;
            }
        }
    }

    out
}

fn build_title_search_variants(input: &str) -> Vec<String> {
    let base = normalize_title_for_igdb_search(input);
    let mut variants = Vec::<String>::new();
    push_unique_variant(&mut variants, base.clone());
    push_unique_variant(&mut variants, expand_common_acronyms(&base));

    let no_year = strip_trailing_year_group(&base);
    push_unique_variant(&mut variants, no_year.clone());
    push_unique_variant(&mut variants, expand_common_acronyms(&no_year));

    let no_suffix = strip_known_title_suffixes(&no_year);
    push_unique_variant(&mut variants, no_suffix.clone());
    push_unique_variant(
        &mut variants,
        strip_known_title_suffixes(&expand_common_acronyms(&no_year)),
    );

    let simplified = no_suffix
        .replace('/', " ")
        .replace(':', " ")
        .replace('-', " ");
    push_unique_variant(&mut variants, simplified.clone());
    push_unique_variant(&mut variants, strip_known_title_suffixes(&simplified));

    if let Some((head, _)) = no_suffix.split_once(':') {
        push_unique_variant(&mut variants, head.trim().to_string());
    }
    if let Some((head, _)) = no_suffix.split_once(" - ") {
        push_unique_variant(&mut variants, head.trim().to_string());
    }

    // Common publisher-prefix trims that often block matching.
    let expanded = expand_common_acronyms(&no_suffix);
    let expanded_lower = expanded.to_ascii_lowercase();
    for prefix in ["tom clancy's ", "tom clancys ", "tc's ", "tcs ", "tc "] {
        if expanded_lower.starts_with(prefix) {
            push_unique_variant(&mut variants, expanded[prefix.len()..].trim().to_string());
        }
    }

    variants
}

async fn lookup_game_ids_from_aliases(
    client: &Client,
    client_id: &str,
    token: &str,
    title: &str,
    max_ids: usize,
) -> Result<Vec<i64>> {
    #[derive(Deserialize)]
    struct AltNameResp {
        game: Option<i64>,
    }

    let alt_names_url = "https://api.igdb.com/v4/alternative_names";
    let mut ids = Vec::<i64>::new();
    let mut seen = std::collections::HashSet::<i64>::new();

    let mut alias_queries = build_title_search_variants(title);
    for token_part in token_fallbacks(title) {
        if !alias_queries
            .iter()
            .any(|q| q.eq_ignore_ascii_case(&token_part))
        {
            alias_queries.push(token_part);
        }
    }

    for q in alias_queries {
        for qv in case_variants(&q) {
            let query = format!(
                "fields game,name,comment; search \"{}\"; limit 30;",
                escape_apicalypse_string(&qv)
            );
            let resp = client
                .post(alt_names_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;

            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /alternative_names"));
            }
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "IGDB /alternative_names error: {}",
                    resp.text().await.unwrap_or_default()
                ));
            }

            let rows: Vec<AltNameResp> = resp.json().await?;
            for row in rows {
                if let Some(game_id) = row.game {
                    if seen.insert(game_id) {
                        ids.push(game_id);
                        if ids.len() >= max_ids {
                            return Ok(ids);
                        }
                    }
                }
            }
        }
    }

    Ok(ids)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbCandidate {
    pub igdb_id: i64,
    pub title: String,
    pub cover_url: Option<String>,
    pub release_year: Option<i32>,
}

pub async fn search_candidates(
    client_id: &str,
    token: &str,
    title: &str,
) -> Result<Vec<IgdbCandidate>> {
    #[derive(Deserialize)]
    struct GameResp {
        id: i64,
        name: String,
        first_release_date: Option<i64>,
        cover: Option<CoverObj>,
    }
    #[derive(Deserialize)]
    struct CoverObj {
        image_id: String,
    }

    let client = Client::new();
    let games_url = "https://api.igdb.com/v4/games";
    let mut games: Vec<GameResp> = Vec::new();

    for query_title in build_title_search_variants(title) {
        for query_variant in case_variants(&query_title) {
            let query = format!(
                "fields id,name,first_release_date,cover.image_id; search \"{}\"; limit 8;",
                escape_apicalypse_string(&query_variant)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;

            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "IGDB /games error: {}",
                    resp.text().await.unwrap_or_default()
                ));
            }

            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
        if !games.is_empty() {
            break;
        }
    }

    if games.is_empty() {
        let mut slugs = Vec::<String>::new();
        for v in build_title_search_variants(title) {
            let slug = slugify_for_igdb(&v);
            if !slug.is_empty() && !slugs.iter().any(|s| s == &slug) {
                slugs.push(slug);
            }
        }
        for slug in slugs {
            let query = format!(
                "fields id,name,first_release_date,cover.image_id,slug; where slug = \"{}\"; limit 8;",
                escape_apicalypse_string(&slug)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "IGDB /games error: {}",
                    resp.text().await.unwrap_or_default()
                ));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
    }

    if games.is_empty() {
        for token_part in token_fallbacks(title) {
            let query = format!(
                "fields id,name,first_release_date,cover.image_id; where name ~ *\"{}\"*; limit 8;",
                escape_apicalypse_string(&token_part)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "IGDB /games error: {}",
                    resp.text().await.unwrap_or_default()
                ));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
    }

    if games.is_empty() {
        for phrase in phrase_fallbacks(title) {
            let query = format!(
                "fields id,name,first_release_date,cover.image_id; where name ~ *\"{}\"*; limit 8;",
                escape_apicalypse_string(&phrase)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "IGDB /games error: {}",
                    resp.text().await.unwrap_or_default()
                ));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
    }

    // Last resort: IGDB alternative_names -> game IDs -> games fetch.
    if games.is_empty() {
        let alias_ids = lookup_game_ids_from_aliases(&client, client_id, token, title, 8).await?;
        if !alias_ids.is_empty() {
            let query = format!(
                "fields id,name,first_release_date,cover.image_id; where id = ({}); limit 8;",
                alias_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "IGDB /games error: {}",
                    resp.text().await.unwrap_or_default()
                ));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
            }
        }
    }

    let out = games
        .into_iter()
        .map(|g| {
            let cover_url = g
                .cover
                .as_ref()
                .map(|c| cover_url_from_image_id(&c.image_id));
            let release_year = g.first_release_date.map(release_year_from_unix);
            IgdbCandidate {
                igdb_id: g.id,
                title: g.name,
                cover_url,
                release_year,
            }
        })
        .collect();
    Ok(out)
}

// full-details by IGDB id (same fields as your normal enrich)
pub async fn lookup_enrich_by_id(
    client_id: &str,
    token: &str,
    id: i64,
) -> Result<Option<IgdbEnrich>> {
    let client = Client::new();
    let url = "https://api.igdb.com/v4/games";
    let q = format!(
        "fields id,name,genres,first_release_date,slug,cover.image_id,summary,storyline,aggregated_rating,rating;
         where id = {id}; limit 1;"
    );
    let resp = client
        .post(url)
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .body(q)
        .send()
        .await?;

    if resp.status().as_u16() == 429 {
        return Err(anyhow!("429 Too Many Requests on /games"));
    }
    if !resp.status().is_success() {
        return Err(anyhow!(
            "IGDB /games error: {}",
            resp.text().await.unwrap_or_default()
        ));
    }

    #[derive(Deserialize)]
    struct GameResp {
        id: i64,
        name: String,
        genres: Option<Vec<i64>>,
        first_release_date: Option<i64>,
        slug: Option<String>,
        cover: Option<CoverObj>,
        summary: Option<String>,
        storyline: Option<String>,
        aggregated_rating: Option<f64>,
        rating: Option<f64>,
    }
    #[derive(Deserialize)]
    struct CoverObj {
        image_id: String,
    }
    #[derive(Deserialize)]
    struct GenreResp {
        name: String,
    }

    let mut games: Vec<GameResp> = resp.json().await?;
    let Some(picked) = games.pop() else {
        return Ok(None);
    };

    let cover_url = picked
        .cover
        .as_ref()
        .map(|c| cover_url_from_image_id(&c.image_id));
    let release_year = picked.first_release_date.map(release_year_from_unix);

    // resolve genre names if any
    let names = if let Some(ids) = picked.genres.clone() {
        if ids.is_empty() {
            Vec::<String>::new()
        } else {
            let genres_url = "https://api.igdb.com/v4/genres";
            let cond = format!(
                "where id = ({});",
                ids.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let gq = format!("fields id,name; {} limit 50;", cond);
            let gresp = client
                .post(genres_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(gq)
                .send()
                .await?;
            if gresp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /genres"));
            }
            if !gresp.status().is_success() {
                return Err(anyhow!(
                    "IGDB /genres error: {}",
                    gresp.text().await.unwrap_or_default()
                ));
            }
            let mut names: Vec<String> = gresp
                .json::<Vec<GenreResp>>()
                .await?
                .into_iter()
                .map(|g| g.name)
                .collect();
            names.sort();
            names
        }
    } else {
        Vec::new()
    };

    Ok(Some(IgdbEnrich {
        igdb_id: picked.id,
        igdb_url: picked.slug.as_deref().and_then(igdb_url_from_slug),
        title: picked.name,
        genres: names,
        cover_url,
        release_year,
        summary: picked.summary,
        storyline: picked.storyline,
        agg_rating: picked.aggregated_rating,
        user_rating: picked.rating,
        ttb_main: None,
        ttb_extra: None,
        ttb_complete: None,
        ttb_count: None,
    }))
}

pub async fn get_oauth_token(client_id: &str, client_secret: &str) -> Result<String> {
    let url = "https://id.twitch.tv/oauth2/token";
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "client_credentials"),
    ];
    let resp = Client::new().post(url).form(&params).send().await?;
    if resp.status().as_u16() == 429 {
        return Err(anyhow!("429 Too Many Requests on token"));
    }
    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_default();
        return Err(anyhow!("IGDB auth failed: {}", t));
    }
    let tok: TokenResponse = resp.json().await?;
    Ok(tok.access_token)
}

#[derive(Deserialize, Debug)]
struct GameResp {
    id: i64,
    name: String,
    genres: Option<Vec<i64>>,
    first_release_date: Option<i64>,
    slug: Option<String>,
    cover: Option<CoverObj>,
    summary: Option<String>,
    storyline: Option<String>,
    aggregated_rating: Option<f64>,
    rating: Option<f64>,
}
#[derive(Deserialize, Debug)]
struct CoverObj {
    image_id: String,
}

#[derive(Deserialize, Debug)]
struct GenreResp {
    name: String,
}

#[derive(Deserialize, Debug)]
struct TtbResp {
    // seconds
    hastily: Option<i64>,    // main
    normally: Option<i64>,   // main + extra
    completely: Option<i64>, // 100%
    count: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug)] // add Serialize
pub struct IgdbEnrich {
    pub igdb_id: i64,
    pub igdb_url: Option<String>,
    pub title: String,
    pub genres: Vec<String>,
    pub cover_url: Option<String>,
    pub release_year: Option<i32>,
    pub summary: Option<String>,
    pub storyline: Option<String>,
    pub agg_rating: Option<f64>,
    pub user_rating: Option<f64>,
    pub ttb_main: Option<i64>,
    pub ttb_extra: Option<i64>,
    pub ttb_complete: Option<i64>,
    pub ttb_count: Option<i64>,
}

fn cover_url_from_image_id(image_id: &str) -> String {
    // Use t_cover_big (adjust if you prefer a larger size)
    format!(
        "https://images.igdb.com/igdb/image/upload/t_cover_big/{}.jpg",
        image_id
    )
}

fn release_year_from_unix(ts: i64) -> i32 {
    chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.year())
        .unwrap_or(0) as i32
}

fn igdb_url_from_slug(slug: &str) -> Option<String> {
    let cleaned = slug.trim().trim_matches('/');
    if cleaned.is_empty() {
        None
    } else {
        Some(format!("https://www.igdb.com/games/{}", cleaned))
    }
}

// Helper to get a year from a Unix epoch (seconds) without deprecated APIs
fn year_from_unix(ts: i64) -> i32 {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.year())
        .unwrap_or(0)
}

pub async fn lookup_enrich_for_title(
    client_id: &str,
    token: &str,
    title: &str,
) -> Result<Option<IgdbEnrich>> {
    let client = Client::new();

    // 1) Games search with richer fields
    let games_url = "https://api.igdb.com/v4/games";
    let mut games: Vec<GameResp> = Vec::new();

    for query_title in build_title_search_variants(title) {
        for query_variant in case_variants(&query_title) {
            let query = format!(
                "fields id,name,genres,first_release_date,slug,cover.image_id,summary,storyline,aggregated_rating,rating; search \"{}\"; limit 5;",
                escape_apicalypse_string(&query_variant)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;

            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                let t = resp.text().await.unwrap_or_default();
                return Err(anyhow!("IGDB /games error: {}", t));
            }

            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
        if !games.is_empty() {
            break;
        }
    }

    if games.is_empty() {
        let mut slugs = Vec::<String>::new();
        for v in build_title_search_variants(title) {
            let slug = slugify_for_igdb(&v);
            if !slug.is_empty() && !slugs.iter().any(|s| s == &slug) {
                slugs.push(slug);
            }
        }
        for slug in slugs {
            let query = format!(
                "fields id,name,genres,first_release_date,slug,cover.image_id,summary,storyline,aggregated_rating,rating; where slug = \"{}\"; limit 5;",
                escape_apicalypse_string(&slug)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                let t = resp.text().await.unwrap_or_default();
                return Err(anyhow!("IGDB /games error: {}", t));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
    }

    if games.is_empty() {
        for token_part in token_fallbacks(title) {
            let query = format!(
                "fields id,name,genres,first_release_date,slug,cover.image_id,summary,storyline,aggregated_rating,rating; where name ~ *\"{}\"*; limit 5;",
                escape_apicalypse_string(&token_part)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                let t = resp.text().await.unwrap_or_default();
                return Err(anyhow!("IGDB /games error: {}", t));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
    }

    if games.is_empty() {
        for phrase in phrase_fallbacks(title) {
            let query = format!(
                "fields id,name,genres,first_release_date,slug,cover.image_id,summary,storyline,aggregated_rating,rating; where name ~ *\"{}\"*; limit 5;",
                escape_apicalypse_string(&phrase)
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                let t = resp.text().await.unwrap_or_default();
                return Err(anyhow!("IGDB /games error: {}", t));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
                break;
            }
        }
    }

    // Last resort: alternative_names -> game IDs -> rich games fetch.
    if games.is_empty() {
        let alias_ids = lookup_game_ids_from_aliases(&client, client_id, token, title, 5).await?;
        if !alias_ids.is_empty() {
            let query = format!(
                "fields id,name,genres,first_release_date,slug,cover.image_id,summary,storyline,aggregated_rating,rating; where id = ({}); limit 5;",
                alias_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let resp = client
                .post(games_url)
                .header("Client-ID", client_id)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .body(query)
                .send()
                .await?;
            if resp.status().as_u16() == 429 {
                return Err(anyhow!("429 Too Many Requests on /games"));
            }
            if !resp.status().is_success() {
                let t = resp.text().await.unwrap_or_default();
                return Err(anyhow!("IGDB /games error: {}", t));
            }
            let found: Vec<GameResp> = resp.json().await?;
            if !found.is_empty() {
                games = found;
            }
        }
    }

    if games.is_empty() {
        return Ok(None);
    }

    let picked = games.remove(0);

    // Cover
    let cover_url = picked
        .cover
        .as_ref()
        .map(|c| cover_url_from_image_id(&c.image_id));

    // Release year
    let release_year = picked.first_release_date.map(year_from_unix);

    // Genres -> names
    let ids = picked.genres.clone().unwrap_or_default();
    let names = if ids.is_empty() {
        Vec::<String>::new()
    } else {
        let genres_url = "https://api.igdb.com/v4/genres";
        let cond = format!(
            "where id = ({});",
            ids.iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let gq = format!("fields id,name; {} limit 50;", cond);
        let gresp = client
            .post(genres_url)
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .body(gq)
            .send()
            .await?;
        if gresp.status().as_u16() == 429 {
            return Err(anyhow!("429 Too Many Requests on /genres"));
        }
        if !gresp.status().is_success() {
            let t = gresp.text().await.unwrap_or_default();
            return Err(anyhow!("IGDB /genres error: {}", t));
        }
        let mut names: Vec<String> = gresp
            .json::<Vec<GenreResp>>()
            .await?
            .into_iter()
            .map(|g| g.name)
            .collect();
        names.sort();
        names
    };

    // 2) Time to beat for this game id
    let (ttb_main, ttb_extra, ttb_complete, ttb_count) = {
        let ttb_url = "https://api.igdb.com/v4/game_time_to_beats";
        let ttb_query = format!(
            "fields game_id,hastily,normally,completely,count; where game_id = {}; limit 1;",
            picked.id
        );
        let ttb_resp = client
            .post(ttb_url)
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .body(ttb_query)
            .send()
            .await?;
        if ttb_resp.status().as_u16() == 429 {
            return Err(anyhow!("429 Too Many Requests on /game_time_to_beats"));
        }
        let ttb: Vec<TtbResp> = if ttb_resp.status().is_success() {
            ttb_resp.json().await?
        } else {
            Vec::new()
        };
        if let Some(row) = ttb.into_iter().next() {
            (row.hastily, row.normally, row.completely, row.count)
        } else {
            (None, None, None, None)
        }
    };

    Ok(Some(IgdbEnrich {
        igdb_id: picked.id,
        igdb_url: picked.slug.as_deref().and_then(igdb_url_from_slug),
        title: picked.name,
        genres: names,
        cover_url,
        release_year,
        summary: picked.summary,
        storyline: picked.storyline,
        agg_rating: picked.aggregated_rating,
        user_rating: picked.rating,
        ttb_main,
        ttb_extra,
        ttb_complete,
        ttb_count,
    }))
}
