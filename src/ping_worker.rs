use std::net::IpAddr;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime};

use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence, SurgeError};
use tokio::sync::OnceCell;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::ping::{PingError, PingResult};

const PING_TIMEOUT_SECS: u64 = 5;
const PING_IDENTIFIER: PingIdentifier = PingIdentifier(1);
const PING_SEQUENCE: PingSequence = PingSequence(1);

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

/// A single ping request handed over to the worker thread
struct PingCommand {
    target: PingTarget,
    timestamp: SystemTime,
}

enum PingTarget {
    /// Already resolved address, no name lookup needed
    Resolved(IpAddr),
    /// Hostname that has to be resolved first
    Hostname(String),
}

/// Handle to the background thread that owns the Tokio runtime and the ICMP socket.
///
/// Dropping the handle closes the command channel, which lets the worker thread
/// finish once its in-flight pings are done.
pub struct PingWorker {
    command_sender: UnboundedSender<PingCommand>,
}

impl PingWorker {
    /// Starts the worker thread. Results are delivered through `result_sender`.
    pub fn spawn(result_sender: Sender<PingResult>) -> Self {
        let (command_sender, command_receiver) = unbounded_channel();
        thread::spawn(move || run_worker(command_receiver, result_sender));
        Self { command_sender }
    }

    /// Pings an address that has already been resolved
    pub fn ping_with_ip(&self, target_ip: IpAddr, timestamp: SystemTime) {
        self.send(PingCommand {
            target: PingTarget::Resolved(target_ip),
            timestamp,
        });
    }

    /// Resolves the target and pings the resulting address
    pub fn resolve_and_ping(&self, target: String, timestamp: SystemTime) {
        self.send(PingCommand {
            target: PingTarget::Hostname(target),
            timestamp,
        });
    }

    fn send(&self, command: PingCommand) {
        if self.command_sender.send(command).is_err() {
            log::error!("ping worker thread is no longer running");
        }
    }
}

/// Worker thread entry point: owns the runtime for the whole lifetime of the app
fn run_worker(mut commands: UnboundedReceiver<PingCommand>, results: Sender<PingResult>) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            log::error!("failed to create the ping runtime: {e}");
            // Answer pending requests so the UI does not wait for results forever
            while let Some(command) = commands.blocking_recv() {
                let error = PingError::RuntimeInit(e.to_string());
                let _ = results.send(PingResult::failure(command.timestamp, error));
            }
            return;
        }
    };

    runtime.block_on(async move {
        let session = Arc::new(PingSession::default());

        while let Some(command) = commands.recv().await {
            // Spawn each ping so a timing out request does not delay the next one
            let session = Arc::clone(&session);
            let results = results.clone();
            tokio::spawn(async move {
                let _ = results.send(session.execute(command).await);
            });
        }
    });
}

/// Shared ICMP state that outlives a single ping
#[derive(Default)]
struct PingSession {
    client: OnceCell<Client>,
}

impl PingSession {
    async fn execute(&self, command: PingCommand) -> PingResult {
        let timestamp = command.timestamp;

        let (target_ip, hostname) = match command.target {
            PingTarget::Resolved(ip) => (ip, None),
            PingTarget::Hostname(target) => match resolve_target(&target).await {
                Some(ip) => (ip, Some(target)),
                None => {
                    log::warn!("failed to resolve target '{target}'");
                    return PingResult::failure(timestamp, PingError::DnsResolution);
                }
            },
        };

        let client = match self.client().await {
            Ok(client) => client,
            Err(e) => {
                log::warn!("failed to create ICMP socket: {e}");
                return PingResult::failure(timestamp, PingError::SocketCreation(e.to_string()));
            }
        };

        let mut pinger = client.pinger(target_ip, PING_IDENTIFIER).await;
        pinger.timeout(Duration::from_secs(PING_TIMEOUT_SECS));

        match pinger.ping(PING_SEQUENCE, &[]).await {
            Ok((IcmpPacket::V4(_), duration)) | Ok((IcmpPacket::V6(_), duration)) => {
                let response_time_ms = duration.as_secs_f64() * 1000.0;
                let resolved_ip = hostname.map(|h| (h, target_ip));
                PingResult::success(timestamp, response_time_ms, resolved_ip)
            }
            Err(e) => {
                log::warn!("ping to {target_ip} failed: {e}");
                PingResult::failure(timestamp, classify_error(e))
            }
        }
    }

    /// Returns the shared client, creating the ICMP socket on first use.
    /// A failed creation is not cached, so the next ping retries it.
    ///
    /// The client must never be cloned out of the cell: dropping any clone
    /// tears down the reply dispatcher shared by all of them.
    async fn client(&self) -> std::io::Result<&Client> {
        self.client
            .get_or_try_init(|| async { Client::new(&Config::default()) })
            .await
    }
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
        Err(e) => {
            log::warn!("lookup of '{sanitized}' failed: {e}");
            None
        }
    }
}

/// Map a surge-ping error to the reason shown in the UI
fn classify_error(error: SurgeError) -> PingError {
    match error {
        SurgeError::Timeout { .. } => PingError::Timeout,
        other => PingError::Network(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    const RESULT_TIMEOUT: Duration = Duration::from_secs(8);

    /// Regression test: the worker keeps one shared `Client`, and cloning it
    /// out would destroy the reply dispatcher after the first ping.
    ///
    /// Skipped when the environment does not allow opening an ICMP socket.
    #[test]
    fn worker_serves_more_than_one_ping() {
        let (sender, receiver) = channel();
        let worker = PingWorker::spawn(sender);
        let localhost = IpAddr::from([127, 0, 0, 1]);

        worker.ping_with_ip(localhost, SystemTime::now());
        let first = receiver.recv_timeout(RESULT_TIMEOUT).expect("no first result");
        if matches!(first.error, Some(PingError::SocketCreation(_))) {
            eprintln!("skipping: {}", first.error.unwrap());
            return;
        }
        assert!(first.success, "first ping failed: {:?}", first.error);

        worker.ping_with_ip(localhost, SystemTime::now());
        let second = receiver.recv_timeout(RESULT_TIMEOUT).expect("no second result");
        assert!(second.success, "second ping failed: {:?}", second.error);
    }
}
