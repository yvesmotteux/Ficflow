use std::sync::RwLock;
use once_cell::sync::Lazy;

const DEFAULT_AO3_URL: &str = "https://archiveofourown.org";
const ALT_AO3_URL: &str = "https://archiveofourown.gay";
const PROXY_AO3_URL: &str = "https://xn--iao3-lw4b.ws";

static AO3_BASE_URL: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(DEFAULT_AO3_URL.to_string()));

pub fn get_ao3_base_url() -> String {
    AO3_BASE_URL.read().unwrap().clone()
}

pub fn set_ao3_base_url(url: &str) {
    let mut ao3_url = AO3_BASE_URL.write().unwrap();
    *ao3_url = url.to_string();
}

pub fn reset_ao3_base_url() {
    let mut ao3_url = AO3_BASE_URL.write().unwrap();
    *ao3_url = DEFAULT_AO3_URL.to_string();
}

pub fn switch_to_alt_ao3_url() {
    set_ao3_base_url(ALT_AO3_URL);
}

pub fn switch_to_proxy_ao3_url() {
    set_ao3_base_url(PROXY_AO3_URL);
}
