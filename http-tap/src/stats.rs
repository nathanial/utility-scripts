use std::collections::HashMap;
use std::time::{SystemTime, Duration};

use hyper::http::Method;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct StatsEvent {
    pub method: Method,
    pub path: String,
    pub at: SystemTime,
}

#[derive(Default, Clone, Debug)]
pub struct MethodCounts {
    pub get: u64,
    pub post: u64,
    pub put: u64,
    pub patch: u64,
    pub delete_: u64,
    pub other: u64,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub path: String,
    pub counts: MethodCounts,
    pub last_seen: SystemTime,
}

#[derive(Default)]
pub struct Aggregator {
    // key: path
    map: HashMap<String, Record>,
}

impl Aggregator {
    pub fn apply(&mut self, ev: StatsEvent) {
        let rec = self.map.entry(ev.path.clone()).or_insert_with(|| Record {
            path: ev.path.clone(),
            counts: MethodCounts::default(),
            last_seen: ev.at,
        });

        match ev.method {
            Method::GET => rec.counts.get += 1,
            Method::POST => rec.counts.post += 1,
            Method::PUT => rec.counts.put += 1,
            Method::PATCH => rec.counts.patch += 1,
            Method::DELETE => rec.counts.delete_ += 1,
            _ => rec.counts.other += 1,
        }
        rec.last_seen = ev.at;
    }

    pub fn snapshot(&self) -> Vec<Record> {
        let mut v: Vec<_> = self.map.values().cloned().collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.last_seen));
        v
    }
}

pub type StatsSender = mpsc::UnboundedSender<StatsEvent>;
pub type StatsReceiver = mpsc::UnboundedReceiver<StatsEvent>;

pub fn channel() -> (StatsSender, StatsReceiver) {
    mpsc::unbounded_channel()
}

