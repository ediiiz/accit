//! Sessions exist only to carry the CSRF token: in memory, at most [`CAPACITY`], the oldest evicted
//! first, [`LIFETIME`] after creation, lost on restart (docs/spec.md section 8).

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use axum::http::{HeaderMap, header};
use subtle::ConstantTimeEq;

/// `__Host-` makes the browser insist on `Secure`, `Path=/` and no `Domain`: a host-only cookie.
pub(super) const COOKIE: &str = "__Host-accit";
pub(super) const LIFETIME: Duration = Duration::from_mins(15);
const CAPACITY: usize = 10_000;
const RANDOM_BYTES: usize = 32;

#[derive(Debug)]
struct Entry {
    token: String,
    created: SystemTime,
}

#[derive(Debug, Default)]
struct Table {
    by_id: HashMap<String, Entry>,
    /// Ids, oldest first.
    order: VecDeque<String>,
}

#[derive(Debug, Default)]
pub(super) struct Sessions(Mutex<Table>);

/// A session's CSRF token, and its id when this call started it and the browser must be given it.
#[derive(Debug)]
pub(super) struct Session {
    pub token: String,
    pub started: Option<String>,
}

fn random_hex() -> Result<String, getrandom::Error> {
    let mut bytes = [0_u8; RANDOM_BYTES];
    getrandom::fill(&mut bytes)?;

    Ok(bytes.iter().fold(String::with_capacity(RANDOM_BYTES * 2), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    }))
}

/// The session id in the request's `Cookie` headers.
pub(super) fn cookie_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(name, id)| (name == COOKIE).then_some(id))
}

pub(super) fn tokens_match(sent: &str, expected: &str) -> bool {
    sent.as_bytes().ct_eq(expected.as_bytes()).into()
}

impl Table {
    fn live(&self, id: &str, now: SystemTime) -> Option<&Entry> {
        self.by_id.get(id).filter(|entry| entry.created + LIFETIME > now)
    }

    fn pop_oldest(&mut self) {
        if let Some(id) = self.order.pop_front() {
            self.by_id.remove(&id);
        }
    }

    fn make_room(&mut self, now: SystemTime) {
        while let Some(entry) = self.order.front().and_then(|id| self.by_id.get(id)) {
            if entry.created + LIFETIME > now {
                break;
            }

            self.pop_oldest();
        }

        while self.order.len() >= CAPACITY {
            self.pop_oldest();
        }
    }
}

impl Sessions {
    fn table(&self) -> std::sync::MutexGuard<'_, Table> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The CSRF token of a live session.
    pub(super) fn token(&self, id: &str, now: SystemTime) -> Option<String> {
        self.table().live(id, now).map(|entry| entry.token.clone())
    }

    /// The live session `id` names, or a new one.
    pub(super) fn resume_or_start(
        &self,
        id: Option<&str>,
        now: SystemTime,
    ) -> Result<Session, getrandom::Error> {
        if let Some(token) = id.and_then(|id| self.token(id, now)) {
            return Ok(Session { token, started: None });
        }

        let (id, token) = (random_hex()?, random_hex()?);
        let mut table = self.table();

        table.make_room(now);
        table.order.push_back(id.clone());
        table.by_id.insert(id.clone(), Entry { token: token.clone(), created: now });

        Ok(Session { token, started: Some(id) })
    }
}
