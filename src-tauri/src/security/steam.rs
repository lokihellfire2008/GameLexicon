use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "GameLexicon";

pub fn save_steam(api_key: &str, profile: &str) -> Result<()> {
    let key_entry =
        Entry::new(SERVICE, "steam_api_key").context("creating keyring entry for steam_api_key")?;
    key_entry
        .set_password(api_key)
        .context("saving steam_api_key")?;

    let profile_entry =
        Entry::new(SERVICE, "steam_profile").context("creating keyring entry for steam_profile")?;
    profile_entry
        .set_password(profile)
        .context("saving steam_profile")?;

    Ok(())
}

pub fn load_steam() -> Result<(Option<String>, Option<String>)> {
    let api_key = match Entry::new(SERVICE, "steam_api_key") {
        Ok(e) => e.get_password().ok(),
        Err(_) => None,
    };

    let profile = match Entry::new(SERVICE, "steam_profile") {
        Ok(e) => e.get_password().ok(),
        Err(_) => None,
    };

    Ok((api_key, profile))
}
