use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Duration, Utc};
use reqwest::StatusCode;

use crate::domain::fanfiction::{Fanfiction, ReadingStatus};
use crate::error::FicflowError;

#[derive(Clone, Copy, Debug)]
pub struct RefreshSchedule {
    pub idle_poll: std::time::Duration,
    pub frequent_every: Duration,
    pub occasional_every: Duration,
    pub occasional_max_age: Duration,
    pub startup_skip_recent: Duration,
    pub max_failures: u32,
    pub pause_when_blocked: Duration,
}

impl Default for RefreshSchedule {
    fn default() -> Self {
        Self {
            idle_poll: std::time::Duration::from_secs(30),
            frequent_every: Duration::minutes(10),
            occasional_every: Duration::minutes(10),
            occasional_max_age: Duration::days(14),
            startup_skip_recent: Duration::hours(1),
            max_failures: 3,
            pause_when_blocked: Duration::minutes(30),
        }
    }
}

pub fn is_followed_closely(fic: &Fanfiction) -> bool {
    !fic.complete && fic.reading_status != ReadingStatus::Abandoned
}

#[derive(Debug, PartialEq)]
pub enum FailureReport {
    GaveUp { fic_id: u64, error: String },
    Paused { error: String },
}

pub struct BackgroundRefresher {
    schedule: RefreshSchedule,
    sweep: VecDeque<u64>,
    next_frequent: DateTime<Utc>,
    next_occasional: DateTime<Utc>,
    failures: HashMap<u64, u32>,
    given_up: HashSet<u64>,
    paused_until: Option<DateTime<Utc>>,
    block_reported: bool,
}

impl BackgroundRefresher {
    pub fn new(schedule: RefreshSchedule, now: DateTime<Utc>) -> Self {
        Self {
            schedule,
            sweep: VecDeque::new(),
            next_frequent: now + schedule.frequent_every,
            next_occasional: now,
            failures: HashMap::new(),
            given_up: HashSet::new(),
            paused_until: None,
            block_reported: false,
        }
    }

    pub fn start_sweep(&mut self, fics: &[Fanfiction], now: DateTime<Utc>, skip_recent: bool) {
        let recent = now - self.schedule.startup_skip_recent;
        let mut due: Vec<&Fanfiction> = fics
            .iter()
            .filter(|f| is_followed_closely(f))
            .filter(|f| !skip_recent || f.last_checked_date < recent)
            .collect();
        due.sort_by_key(|f| f.last_checked_date);
        self.sweep = due.into_iter().map(|f| f.id).collect();
    }

    pub fn sweep_remaining(&self) -> usize {
        self.sweep.len()
    }

    pub fn resume(&mut self) {
        self.paused_until = None;
    }

    pub fn forgive(&mut self, fic_id: u64) {
        self.failures.remove(&fic_id);
        self.given_up.remove(&fic_id);
    }

    pub fn schedule(&self) -> RefreshSchedule {
        self.schedule
    }

    pub fn next_in_sweep(&mut self, fics: &[Fanfiction], now: DateTime<Utc>) -> Option<u64> {
        if self.is_paused(now) {
            return None;
        }
        while let Some(fic_id) = self.sweep.pop_front() {
            if !self.given_up.contains(&fic_id) && fics.iter().any(|f| f.id == fic_id) {
                return Some(fic_id);
            }
        }
        None
    }

    pub fn next_due(&mut self, fics: &[Fanfiction], now: DateTime<Utc>) -> Option<u64> {
        if self.is_paused(now) {
            return None;
        }
        if let Some(fic_id) = self.next_in_sweep(fics, now) {
            return Some(fic_id);
        }

        if now >= self.next_frequent {
            self.next_frequent = now + self.schedule.frequent_every;
            if let Some(fic_id) = self.oldest_checked(fics, is_followed_closely) {
                return Some(fic_id);
            }
        }

        if now >= self.next_occasional {
            self.next_occasional = now + self.schedule.occasional_every;
            let stale = now - self.schedule.occasional_max_age;
            if let Some(fic_id) = self.oldest_checked(fics, |f| {
                !is_followed_closely(f) && f.last_checked_date < stale
            }) {
                return Some(fic_id);
            }
        }

        None
    }

    pub fn record_success(&mut self, fic_id: u64) {
        self.failures.remove(&fic_id);
        self.block_reported = false;
    }

    pub fn record_failure(
        &mut self,
        fic_id: u64,
        error: &FicflowError,
        now: DateTime<Utc>,
    ) -> Option<FailureReport> {
        if blocks_every_request(error) {
            self.paused_until = Some(now + self.schedule.pause_when_blocked);
            self.sweep.push_front(fic_id);
            let first_report = !self.block_reported;
            self.block_reported = true;
            return first_report.then(|| FailureReport::Paused {
                error: error.to_string(),
            });
        }
        let failures = self.failures.entry(fic_id).or_insert(0);
        *failures += 1;
        if *failures < self.schedule.max_failures {
            return None;
        }
        self.failures.remove(&fic_id);
        self.given_up.insert(fic_id);
        Some(FailureReport::GaveUp {
            fic_id,
            error: error.to_string(),
        })
    }

    fn is_paused(&mut self, now: DateTime<Utc>) -> bool {
        if self.paused_until.is_some_and(|until| now < until) {
            return true;
        }
        self.paused_until = None;
        false
    }

    fn oldest_checked(
        &self,
        fics: &[Fanfiction],
        eligible: impl Fn(&Fanfiction) -> bool,
    ) -> Option<u64> {
        fics.iter()
            .filter(|f| eligible(f) && !self.given_up.contains(&f.id))
            .min_by_key(|f| f.last_checked_date)
            .map(|f| f.id)
    }
}

fn blocks_every_request(error: &FicflowError) -> bool {
    match error {
        FicflowError::CloudflareChallenge | FicflowError::SessionExpired => true,
        FicflowError::Network(e) => e.status() == Some(StatusCode::TOO_MANY_REQUESTS),
        _ => false,
    }
}
