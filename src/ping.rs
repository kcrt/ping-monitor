use std::time::SystemTime;
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct PingResult {
    pub timestamp: SystemTime,
    pub response_time: Option<f64>,
    pub success: bool,
    pub resolved_ip: Option<(String, IpAddr)>,
}

impl PingResult {
    pub fn success(timestamp: SystemTime, response_time_ms: f64, resolved_ip: Option<(String, IpAddr)>) -> Self {
        Self {
            timestamp,
            response_time: Some(response_time_ms),
            success: true,
            resolved_ip,
        }
    }

    pub fn failure(timestamp: SystemTime) -> Self {
        Self {
            timestamp,
            response_time: None,
            success: false,
            resolved_ip: None,
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
