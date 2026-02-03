use std::net::IpAddr;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DnsCacheEntry {
    ip_address: IpAddr,
    cached_at: SystemTime,
    ttl: Duration,
}

impl DnsCacheEntry {
    pub fn new(ip_address: IpAddr, ttl_seconds: u64) -> Self {
        Self {
            ip_address,
            cached_at: SystemTime::now(),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }
    
    pub fn is_expired(&self) -> bool {
        SystemTime::now()
            .duration_since(self.cached_at)
            .map_or(true, |elapsed| elapsed > self.ttl)
    }

    pub fn ip_address(&self) -> IpAddr {
        self.ip_address
    }
}

pub struct DnsCache {
    cache: HashMap<String, DnsCacheEntry>,
}

impl DnsCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get(&self, hostname: &str) -> Option<&DnsCacheEntry> {
        self.cache.get(hostname)
    }

    pub fn insert(&mut self, hostname: String, entry: DnsCacheEntry) {
        self.cache.insert(hostname, entry);
    }

    pub fn remove(&mut self, hostname: &str) {
        self.cache.remove(hostname);
    }

    pub fn get_valid_ip(&self, hostname: &str) -> Option<IpAddr> {
        self.get(hostname)
            .filter(|entry| !entry.is_expired())
            .map(|entry| entry.ip_address())
    }

    pub fn clean_expired(&mut self, hostname: &str) {
        if let Some(entry) = self.get(hostname) {
            if entry.is_expired() {
                self.remove(hostname);
            }
        }
    }
}
