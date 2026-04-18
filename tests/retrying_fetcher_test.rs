use std::cell::Cell;
use std::time::Duration;

#[path = "common/mod.rs"]
mod common;
use common::fixtures;

use ficflow::{
    domain::fanfiction::{Fanfiction, FanfictionFetcher},
    error::FicflowError,
    infrastructure::RetryingFetcher,
};

struct FlakyFetcher {
    calls: Cell<u32>,
    fail_first: u32,
}

impl FanfictionFetcher for FlakyFetcher {
    fn fetch_fanfiction(&self, fic_id: u64, _base_url: &str) -> Result<Fanfiction, FicflowError> {
        let n = self.calls.get();
        self.calls.set(n + 1);
        if n < self.fail_first {
            Err(FicflowError::Parse { field: "title".into(), reason: "flaky".into() })
        } else {
            Ok(fixtures::given_sample_fanfiction(fic_id, "sample"))
        }
    }
}

#[test]
fn retries_parse_errors_and_eventually_succeeds() {
    let flaky = FlakyFetcher { calls: Cell::new(0), fail_first: 2 };
    let retrying = RetryingFetcher::with_backoff(flaky, 3, Duration::from_millis(1));
    assert!(retrying.fetch_fanfiction(42, "http://test").is_ok());
}

#[test]
fn surfaces_error_after_exhausting_retries() {
    let flaky = FlakyFetcher { calls: Cell::new(0), fail_first: 10 };
    let retrying = RetryingFetcher::with_backoff(flaky, 2, Duration::from_millis(1));
    assert!(matches!(
        retrying.fetch_fanfiction(42, "http://test"),
        Err(FicflowError::Parse { .. })
    ));
}
