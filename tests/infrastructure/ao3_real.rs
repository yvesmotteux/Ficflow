//! Smoke test against the real archiveofourown.org.
//!
//! Ignored by default so CI stays green when AO3 is down, rate-limiting, or
//! has rolled out a new Cloudflare rule. Run manually with:
//!
//! ```sh
//! cargo test -- --ignored
//! ```

#[cfg(test)]
mod tests {
    use ficflow::{
        domain::fanfiction::FanfictionFetcher, infrastructure::external::ao3::Ao3Fetcher,
    };
    use std::time::Duration;

    // A long-lived public fic. Swap this ID if it ever becomes invalid.
    const STABLE_PUBLIC_FIC_ID: u64 = 53960491;
    const AO3_BASE_URL: &str = "https://archiveofourown.org";

    #[test]
    #[ignore]
    fn real_ao3_accepts_our_request() {
        let fetcher = Ao3Fetcher::with_min_gap(Duration::ZERO)
            .expect("fetcher should build");

        let fic = fetcher
            .fetch_fanfiction(STABLE_PUBLIC_FIC_ID, AO3_BASE_URL)
            .expect("AO3 returned an error — if this is a 403, the fetcher regressed");

        assert_eq!(fic.id, STABLE_PUBLIC_FIC_ID);
        assert!(!fic.title.is_empty(), "parsed title was empty");
        assert!(!fic.authors.is_empty(), "parsed authors were empty");
    }
}
