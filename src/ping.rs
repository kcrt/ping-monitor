use std::fmt;
use std::time::SystemTime;
use std::net::IpAddr;

/// Reason why a ping did not produce a response time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PingError {
    /// The target could not be resolved to an IP address.
    DnsResolution,
    /// The ICMP socket could not be created (e.g. missing privileges).
    SocketCreation(String),
    /// No echo reply arrived before the timeout.
    Timeout,
    /// The echo request could not be sent, or the reply was malformed.
    Network(String),
}

impl fmt::Display for PingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PingError::DnsResolution => write!(f, "DNS resolution failed"),
            PingError::SocketCreation(detail) => write!(f, "ICMP socket creation failed: {detail}"),
            PingError::Timeout => write!(f, "Request timed out"),
            PingError::Network(detail) => write!(f, "Network error: {detail}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PingResult {
    pub timestamp: SystemTime,
    pub response_time: Option<f64>,
    pub success: bool,
    pub resolved_ip: Option<(String, IpAddr)>,
    pub error: Option<PingError>,
}

impl PingResult {
    pub fn success(timestamp: SystemTime, response_time_ms: f64, resolved_ip: Option<(String, IpAddr)>) -> Self {
        Self {
            timestamp,
            response_time: Some(response_time_ms),
            success: true,
            resolved_ip,
            error: None,
        }
    }

    pub fn failure(timestamp: SystemTime, error: PingError) -> Self {
        Self {
            timestamp,
            response_time: None,
            success: false,
            resolved_ip: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PingStatistics {
    pub total_pings: u64,
    pub successful_pings: u64,
    pub failed_pings: u64,
    pub total_response_time: f64,
    pub loss_rate: f64,
    pub mean_response_time: f64,
}
