use std::sync::Arc;

use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{ACCEPT, ORIGIN, REFERER};
use serde::{Deserialize, Serialize};

use crate::error::FicflowError;
use crate::infrastructure::external::ao3::ao3_client::{browser_headers, read_page};

const KEYCHAIN_SERVICE: &str = "ficflow";
const KEYCHAIN_ENTRY: &str = "ao3-session";

const LOGOUT_LINK: &str = "href=\"/users/logout";

#[derive(Deserialize)]
struct TokenDispenser {
    token: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ao3Session {
    pub username: String,
    pub cookie: String,
}

pub trait SessionStore: Send + Sync {
    fn load(&self) -> Option<Ao3Session>;
    fn save(&self, session: &Ao3Session) -> Result<(), FicflowError>;
    fn clear(&self) -> Result<(), FicflowError>;
}

pub struct KeychainSessionStore;

impl KeychainSessionStore {
    fn entry() -> Result<keyring::Entry, FicflowError> {
        keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ENTRY).map_err(keychain_error)
    }
}

impl SessionStore for KeychainSessionStore {
    fn load(&self) -> Option<Ao3Session> {
        let secret = Self::entry()
            .and_then(|entry| entry.get_password().map_err(keychain_error))
            .inspect_err(|err| log::debug!("no AO3 session available: {err}"))
            .ok()?;
        serde_json::from_str(&secret)
            .inspect_err(|err| log::warn!("ignoring unreadable AO3 session: {err}"))
            .ok()
    }

    fn save(&self, session: &Ao3Session) -> Result<(), FicflowError> {
        Self::entry()?
            .set_password(&serde_json::to_string(session)?)
            .map_err(keychain_error)
    }

    fn clear(&self) -> Result<(), FicflowError> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(keychain_error(err)),
        }
    }
}

fn keychain_error(err: keyring::Error) -> FicflowError {
    FicflowError::Keychain(err.to_string())
}

pub fn log_in(base_url: &str, username: &str, password: &str) -> Result<Ao3Session, FicflowError> {
    let site = Url::parse(base_url)
        .map_err(|err| FicflowError::InvalidInput(format!("bad AO3 address: {err}")))?;
    let jar = Arc::new(Jar::default());
    let client = Client::builder()
        .default_headers(browser_headers())
        .cookie_provider(Arc::clone(&jar))
        .http1_only()
        .http1_title_case_headers()
        .build()?;
    let base = base_url.trim_end_matches('/');

    let login_url = format!("{base}/users/login");

    let dispenser = client
        .get(format!("{base}/token_dispenser.json"))
        .header(ACCEPT, "application/json, text/javascript, */*; q=0.01")
        .header("x-requested-with", "XMLHttpRequest")
        .header(REFERER, &login_url)
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-origin")
        .send()?;
    let token = serde_json::from_str::<TokenDispenser>(&read_page(dispenser)?)?.token;

    let response = client
        .post(&login_url)
        .header(ORIGIN, base)
        .header(REFERER, &login_url)
        .header("sec-fetch-site", "same-origin")
        .form(&[
            ("utf8", "\u{2713}"),
            ("authenticity_token", token.as_str()),
            ("user[login]", username),
            ("user[password]", password),
            ("user[remember_me]", "1"),
            ("commit", "Log in"),
        ])
        .send()?;
    let page = read_page(response)?;
    if !page.contains(LOGOUT_LINK) {
        return Err(FicflowError::LoginRejected);
    }

    let cookie = jar
        .cookies(&site)
        .and_then(|value| value.to_str().ok().map(str::to_string))
        .ok_or(FicflowError::LoginRejected)?;
    Ok(Ao3Session {
        username: username.to_string(),
        cookie,
    })
}
