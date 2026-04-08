use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "GameLexicon";

pub fn save_twitch(client_id: &str, client_secret: &str) -> Result<()> {
    let id_entry = Entry::new(SERVICE, "twitch_client_id")
        .context("creating keyring entry for twitch_client_id")?;
    id_entry
        .set_password(client_id)
        .context("saving twitch_client_id")?;

    let secret_entry = Entry::new(SERVICE, "twitch_client_secret")
        .context("creating keyring entry for twitch_client_secret")?;
    secret_entry
        .set_password(client_secret)
        .context("saving twitch_client_secret")?;

    Ok(())
}

pub fn load_twitch() -> Result<(Option<String>, Option<String>)> {
    let id = match Entry::new(SERVICE, "twitch_client_id") {
        Ok(e) => e.get_password().ok(),
        Err(_) => None,
    };

    let secret = match Entry::new(SERVICE, "twitch_client_secret") {
        Ok(e) => e.get_password().ok(),
        Err(_) => None,
    };

    Ok((id, secret))
}
