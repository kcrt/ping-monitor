use std::net::IpAddr;
use std::time::{Duration, SystemTime};
use std::sync::mpsc;
use std::thread;
use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence};
use crate::ping::PingResult;

const PING_TIMEOUT_SECS: u64 = 5;

/// Sanitize hostname by keeping only valid characters (alphanumeric, dots, hyphens)
/// Returns None if the result is empty
fn sanitize_hostname(hostname: &str) -> Option<String> {
    // Also handle case where user included port like "example.com:8080"
    let hostname = hostname.split(':').next().unwrap_or(hostname);

    let sanitized: String = hostname
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-')
        .collect();

    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

pub struct PingExecutor;

impl PingExecutor {
    /// Resolves hostname (if needed) and executes ping asynchronously
    pub fn resolve_and_ping(target: String, sender: mpsc::Sender<PingResult>) {
        let timestamp = SystemTime::now();
        
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                let target_ip = match Self::resolve_target(&target).await {
                    Some(ip) => ip,
                    None => return PingResult::failure(timestamp),
                };

                Self::execute_ping(target_ip, timestamp, Some(target)).await
            });
            
            let _ = sender.send(result);
        });
    }

    /// Executes ping with a pre-resolved IP address
    pub fn ping_with_ip(target_ip: IpAddr, sender: mpsc::Sender<PingResult>) {
        let timestamp = SystemTime::now();
        
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(Self::execute_ping(target_ip, timestamp, None));
            let _ = sender.send(result);
        });
    }

    /// Resolve hostname to IP address
    async fn resolve_target(target: &str) -> Option<IpAddr> {
        // Try parsing as IP address first
        if let Ok(ip) = target.parse::<IpAddr>() {
            return Some(ip);
        }

        // Sanitize hostname input
        let sanitized = sanitize_hostname(target)?;

        // Try resolving as hostname
        match tokio::net::lookup_host(&format!("{sanitized}:80")).await {
            Ok(mut addrs) => addrs.next().map(|addr| addr.ip()),
            Err(_) => None,
        }
    }

    /// Execute the actual ping operation
    async fn execute_ping(
        target_ip: IpAddr, 
        timestamp: SystemTime,
        hostname: Option<String>
    ) -> PingResult {
        let config = Config::default();
        let client = match Client::new(&config) {
            Ok(client) => client,
            Err(_) => return PingResult::failure(timestamp),
        };
        
        let mut pinger = client.pinger(target_ip, PingIdentifier(1)).await;
        pinger.timeout(Duration::from_secs(PING_TIMEOUT_SECS));
        
        match pinger.ping(PingSequence(1), &[]).await {
            Ok((IcmpPacket::V4(_), duration)) | Ok((IcmpPacket::V6(_), duration)) => {
                let response_time_ms = duration.as_secs_f64() * 1000.0;
                let resolved_ip = hostname.map(|h| (h, target_ip));
                PingResult::success(timestamp, response_time_ms, resolved_ip)
            }
            Err(_) => PingResult::failure(timestamp),
        }
    }
}
