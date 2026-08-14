use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use time::{Duration, OffsetDateTime};

use super::auth::Standing;

const REMEMBER_FOR: Duration = Duration::seconds(60);

type Asked = (Option<Standing>, OffsetDateTime);

#[derive(Clone, Default)]
pub struct WhoMaySeeWhat {
    known: Arc<Mutex<HashMap<(String, String), Asked>>>,
}

impl WhoMaySeeWhat {
    pub fn recently(&self, login: &str, repository: &str) -> Option<Option<Standing>> {
        let known = self.known.lock().ok()?;
        let (standing, asked_at) = known.get(&(login.to_owned(), repository.to_owned()))?;
        (OffsetDateTime::now_utc() - *asked_at < REMEMBER_FOR).then_some(*standing)
    }

    pub fn remember(&self, login: &str, repository: &str, standing: Option<Standing>) {
        let Ok(mut known) = self.known.lock() else {
            return;
        };
        let now = OffsetDateTime::now_utc();
        known.retain(|_, (_, asked_at)| now - *asked_at < REMEMBER_FOR);
        known.insert((login.to_owned(), repository.to_owned()), (standing, now));
    }

    pub fn forget(&self, login: &str) {
        if let Ok(mut known) = self.known.lock() {
            known.retain(|(who, _), _| who != login);
        }
    }
}
