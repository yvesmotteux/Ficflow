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
        domain::fanfiction::FanfictionFetcher,
        infrastructure::external::ao3::{fetcher::PRIMARY_AO3_URL, Ao3Fetcher},
    };
    use std::time::Duration;

    // A long-lived public fic. Swap this ID if it ever becomes invalid.
    const STABLE_PUBLIC_FIC_ID: u64 = 53960491;

    #[test]
    #[ignore]
    fn real_ao3_accepts_our_request() {
        let fetcher = Ao3Fetcher::with_min_gap(
            vec![PRIMARY_AO3_URL.to_string()],
            1,
            Duration::ZERO,
            Duration::from_millis(1),
        )
        .expect("fetcher should build");

        let fic = fetcher
            .fetch_fanfiction(STABLE_PUBLIC_FIC_ID)
            .expect("AO3 returned an error — if this is a 403, the fetcher regressed");

        assert_eq!(fic.id, STABLE_PUBLIC_FIC_ID);
        assert!(!fic.title.is_empty(), "parsed title was empty");
        assert!(!fic.authors.is_empty(), "parsed authors were empty");
    }
}
