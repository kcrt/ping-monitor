use std::collections::{VecDeque, HashMap};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::fs;
use eframe::egui;
use egui::{Color32, Vec2, Pos2, Stroke};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;
use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence};
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct PingResult {
    pub timestamp: SystemTime,
    pub response_time: Option<f64>,
    pub success: bool,
    pub resolved_ip: Option<(String, IpAddr)>, // (hostname, ip) for caching
}

#[derive(Debug, Clone, Copy)]
pub enum CircleColor {
    Gray,
    Green,
    Yellow,
    Orange,
    Red,
}

impl CircleColor {
    fn to_color32(self) -> Color32 {
        match self {
            CircleColor::Gray => Color32::GRAY,
            CircleColor::Green => Color32::GREEN,
            CircleColor::Yellow => Color32::YELLOW,
            CircleColor::Orange => Color32::from_rgb(255, 165, 0),
            CircleColor::Red => Color32::RED,
        }
    }
    
    fn to_color32_with_age(self, elapsed_seconds: f64) -> Color32 {
        if elapsed_seconds >= 55.0 {
            return Color32::GRAY;
        }
        
        let base_color = self.to_color32();
        
        if elapsed_seconds <= 35.0 {
            return base_color;
        }
        
        // Fade from full color to gray between 35-55 seconds
        let fade_factor = 1.0 - (elapsed_seconds - 35.0) / 20.0;
        let fade_factor = fade_factor.clamp(0.0, 1.0) as f32;
        
        let gray = Color32::GRAY;
        Color32::from_rgb(
            (base_color.r() as f32 * fade_factor + gray.r() as f32 * (1.0 - fade_factor)) as u8,
            (base_color.g() as f32 * fade_factor + gray.g() as f32 * (1.0 - fade_factor)) as u8,
            (base_color.b() as f32 * fade_factor + gray.b() as f32 * (1.0 - fade_factor)) as u8,
        )
    }
}

pub struct PingMonitorApp {
    pub target: String,
    pub is_monitoring: bool,
    pub ping_results: VecDeque<PingResult>,
    pub circles: [CircleColor; 12],
    pub circle_timestamps: [Option<SystemTime>; 12],
    pub last_ping_second: Option<u64>,
    pub ping_statistics: PingStatistics,
    pub ping_receiver: Option<mpsc::Receiver<PingResult>>,
    pub ping_sender: Option<mpsc::Sender<PingResult>>,
    pub pending_pings: std::collections::HashMap<usize, SystemTime>,
    pub dns_cache: HashMap<String, DnsCacheEntry>,
    pub green_threshold: u64,
    pub yellow_threshold: u64,
    pub last_response_time: Option<f64>,
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

#[derive(Debug, Clone)]
pub struct DnsCacheEntry {
    ip_address: IpAddr,
    cached_at: SystemTime,
    ttl: Duration,
}

impl DnsCacheEntry {
    fn new(ip_address: IpAddr, ttl_seconds: u64) -> Self {
        Self {
            ip_address,
            cached_at: SystemTime::now(),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }
    
    fn is_expired(&self) -> bool {
        SystemTime::now()
            .duration_since(self.cached_at)
            .map_or(true, |elapsed| elapsed > self.ttl)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    target: String,
    green_threshold: u64,
    yellow_threshold: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            target: "8.8.8.8".to_string(),
            green_threshold: 100,
            yellow_threshold: 200,
        }
    }
}

impl Default for PingMonitorApp {
    fn default() -> Self {
        Self {
            target: "8.8.8.8".to_string(),
            is_monitoring: false,
            ping_results: VecDeque::new(),
            circles: [CircleColor::Gray; 12],
            circle_timestamps: [None; 12],
            last_ping_second: None,
            ping_statistics: PingStatistics::default(),
            ping_receiver: None,
            ping_sender: None,
            pending_pings: HashMap::new(),
            dns_cache: HashMap::new(),
            green_threshold: 100,
            yellow_threshold: 200,
            last_response_time: None,
        }
    }
}

impl PingMonitorApp {
    pub fn new() -> Self {
        let config = Self::load_config();
        Self {
            target: config.target,
            is_monitoring: false,
            ping_results: VecDeque::new(),
            circles: [CircleColor::Gray; 12],
            circle_timestamps: [None; 12],
            last_ping_second: None,
            ping_statistics: PingStatistics::default(),
            ping_receiver: None,
            ping_sender: None,
            pending_pings: HashMap::new(),
            dns_cache: HashMap::new(),
            green_threshold: config.green_threshold,
            yellow_threshold: config.yellow_threshold,
            last_response_time: None,
        }
    }

    fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("PingMonitor");
        
        fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("config.json"))
    }

    fn load_config() -> AppConfig {
        match Self::get_config_path() {
            Ok(path) => {
                if path.exists() {
                    match fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<AppConfig>(&content) {
                                Ok(config) => return config,
                                Err(e) => eprintln!("Failed to parse config: {e}"),
                            }
                        }
                        Err(e) => eprintln!("Failed to read config file: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("Failed to get config path: {e}"),
        }
        AppConfig::default()
    }

    fn save_config(&self) {
        let config = AppConfig {
            target: self.target.clone(),
            green_threshold: self.green_threshold,
            yellow_threshold: self.yellow_threshold,
        };

        match Self::get_config_path() {
            Ok(path) => {
                match serde_json::to_string_pretty(&config) {
                    Ok(content) => {
                        if let Err(e) = fs::write(&path, content) {
                            eprintln!("Failed to save config: {e}");
                        }
                    }
                    Err(e) => eprintln!("Failed to serialize config: {e}"),
                }
            }
            Err(e) => eprintln!("Failed to get config path: {e}"),
        }
    }


    fn resolve_and_ping_async(&mut self, target: String, _circle_index: usize, sender: mpsc::Sender<PingResult>) {
        let timestamp = SystemTime::now();
        
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                // Parse target as IP address or resolve hostname
                let target_ip: IpAddr = match target.parse() {
                    Ok(ip) => ip,
                    Err(_) => {
                        // Try to resolve hostname
                        match tokio::net::lookup_host(&format!("{target}:80")).await {
                            Ok(mut addrs) => {
                                if let Some(addr) = addrs.next() {
                                    addr.ip()
                                } else {
                                    return PingResult {
                                        timestamp,
                                        response_time: None,
                                        success: false,
                                        resolved_ip: None,
                                    };
                                }
                            }
                            Err(_) => return PingResult {
                                timestamp,
                                response_time: None,
                                success: false,
                                resolved_ip: None,
                            },
                        }
                    }
                };

                let config = Config::default();
                let client = Client::new(&config);
                
                match client {
                    Ok(client) => {
                        let mut pinger = client.pinger(target_ip, PingIdentifier(1)).await;
                        pinger.timeout(Duration::from_secs(5));
                        
                        match pinger.ping(PingSequence(1), &[]).await {
                            Ok((IcmpPacket::V4(_packet), duration)) => {
                                PingResult {
                                    timestamp,
                                    response_time: Some(duration.as_secs_f64() * 1000.0),
                                    success: true,
                                    resolved_ip: Some((target.clone(), target_ip)),
                                }
                            }
                            Ok((IcmpPacket::V6(_packet), duration)) => {
                                PingResult {
                                    timestamp,
                                    response_time: Some(duration.as_secs_f64() * 1000.0),
                                    success: true,
                                    resolved_ip: Some((target.clone(), target_ip)),
                                }
                            }
                            Err(_) => PingResult {
                                timestamp,
                                response_time: None,
                                success: false,
                                resolved_ip: None,
                            },
                        }
                    }
                    Err(_) => PingResult {
                        timestamp,
                        response_time: None,
                        success: false,
                        resolved_ip: None,
                    },
                }
            });
            
            let _ = sender.send(result);
        });
    }

    fn start_async_ping_with_ip(&self, target_ip: IpAddr, _circle_index: usize, sender: mpsc::Sender<PingResult>) {
        let timestamp = SystemTime::now();
        
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {

                let config = Config::default();
                let client = Client::new(&config);
                
                match client {
                    Ok(client) => {
                        let mut pinger = client.pinger(target_ip, PingIdentifier(1)).await;
                        pinger.timeout(Duration::from_secs(5));
                        
                        match pinger.ping(PingSequence(1), &[]).await {
                            Ok((IcmpPacket::V4(_packet), duration)) => {
                                PingResult {
                                    timestamp,
                                    response_time: Some(duration.as_secs_f64() * 1000.0),
                                    success: true,
                                    resolved_ip: None, // This function uses pre-resolved IP
                                }
                            }
                            Ok((IcmpPacket::V6(_packet), duration)) => {
                                PingResult {
                                    timestamp,
                                    response_time: Some(duration.as_secs_f64() * 1000.0),
                                    success: true,
                                    resolved_ip: None, // This function uses pre-resolved IP
                                }
                            }
                            Err(_) => PingResult {
                                timestamp,
                                response_time: None,
                                success: false,
                                resolved_ip: None,
                            },
                        }
                    }
                    Err(_) => PingResult {
                        timestamp,
                        response_time: None,
                        success: false,
                        resolved_ip: None,
                    },
                }
            });
            
            let _ = sender.send(result);
        });
    }


    fn get_circle_color(&self, ping_result: &PingResult) -> CircleColor {
        if !ping_result.success {
            return CircleColor::Red;
        }
        
        match ping_result.response_time {
            Some(time) => {
                if time < self.green_threshold as f64 {
                    CircleColor::Green
                } else if time < self.yellow_threshold as f64 {
                    CircleColor::Yellow
                } else {
                    CircleColor::Orange
                }
            }
            None => CircleColor::Red,
        }
    }

    fn update_statistics(&mut self) {
        let now = SystemTime::now();
        let cutoff_time = now - Duration::from_secs(60);
        
        // Filter ping results to only include those from the last 60 seconds
        let recent_results: Vec<&PingResult> = self.ping_results
            .iter()
            .filter(|r| r.timestamp >= cutoff_time)
            .collect();
        
        let total = recent_results.len() as u64;
        let successful = recent_results.iter().filter(|r| r.success).count() as u64;
        let failed = total - successful;
        
        let total_response_time: f64 = recent_results
            .iter()
            .filter_map(|r| r.response_time)
            .sum();
        
        self.ping_statistics = PingStatistics {
            total_pings: total,
            successful_pings: successful,
            failed_pings: failed,
            total_response_time,
            loss_rate: if total > 0 { (failed as f64 / total as f64) * 100.0 } else { 0.0 },
            mean_response_time: if successful > 0 { total_response_time / successful as f64 } else { 0.0 },
        };
    }

    fn get_circle_index_for_time(time: SystemTime) -> usize {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
        let seconds = duration.as_secs();
        ((seconds % 60) / 5) as usize
    }

    fn draw_clock_face(&self, ui: &mut egui::Ui) {
        let available_rect = ui.available_rect_before_wrap();
        let center = available_rect.center();
        let radius = 100.0;
        let circle_radius = 10.0;
        
        let painter = ui.painter();
        
        fn place_in_circle(center: Pos2, radius: f32, angle: f32) -> Pos2 {
            Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        }

        for i in 0..12 {
            let angle = (i as f32 * 30.0 - 90.0) * std::f32::consts::PI / 180.0;
            let pos = place_in_circle(center, radius, angle);
            let color = if let Some(timestamp) = self.circle_timestamps[i] {
                if let Ok(elapsed) = SystemTime::now().duration_since(timestamp) {
                    let elapsed_seconds = elapsed.as_secs_f64();
                    self.circles[i].to_color32_with_age(elapsed_seconds)
                } else {
                    self.circles[i].to_color32()
                }
            } else {
                self.circles[i].to_color32()
            };
            painter.circle_filled(pos, circle_radius, color);
            
            let stroke_color = if self.pending_pings.contains_key(&i) {
                Color32::RED
            } else {
                Color32::BLACK
            };
            painter.circle_stroke(pos, circle_radius, Stroke::new(2.0, stroke_color));
            
            let text = format!("{}", i * 5);
            let text_pos = place_in_circle(center, radius - 25.0, angle);
            let font_size = 12.0;
            let font = egui::FontId::monospace(font_size);
            painter.text(text_pos, egui::Align2::CENTER_CENTER, text, font, ui.visuals().text_color());
        }
        
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
        let total_ms = duration.as_millis() % 60000;
        let second_angle = (total_ms as f32 * 6.0 / 1000.0 - 90.0) * std::f32::consts::PI / 180.0;
        let hand_length = radius * 0.8;
        let hand_end = Pos2::new(
            center.x + hand_length * second_angle.cos(),
            center.y + hand_length * second_angle.sin()
        );
        
        painter.line_segment([center, hand_end], Stroke::new(3.0, Color32::RED));
        painter.circle_filled(center, 4.0, Color32::RED);
    }
}

impl eframe::App for PingMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let previous_target = self.target.clone();
        let previous_green = self.green_threshold;
        let previous_yellow = self.yellow_threshold;
        
        // Handle incoming ping results
        let mut ping_results_to_process = Vec::new();
        if let Some(receiver) = &self.ping_receiver {
            while let Ok(ping_result) = receiver.try_recv() {
                ping_results_to_process.push(ping_result);
            }
        }
        
        for ping_result in ping_results_to_process {
            let circle_index = Self::get_circle_index_for_time(ping_result.timestamp);
            self.circles[circle_index] = self.get_circle_color(&ping_result);
            self.circle_timestamps[circle_index] = Some(ping_result.timestamp);
            
            // Update last response time
            self.last_response_time = ping_result.response_time;
            
            // Update DNS cache if we have resolution info
            if let Some((hostname, ip)) = &ping_result.resolved_ip {
                if hostname != &ip.to_string() { // Only cache actual hostnames, not IP addresses
                    self.dns_cache.insert(hostname.clone(), DnsCacheEntry::new(*ip, 300)); // 5-minute TTL
                }
            }
            
            self.ping_results.push_back(ping_result);
            
            if self.ping_results.len() > 60 {
                self.ping_results.pop_front();
            }
            
            self.update_statistics();
            
            // Remove from pending pings
            self.pending_pings.remove(&circle_index);
        }
        
        // Clean up old pending pings (timeout after 10 seconds)
        let now = SystemTime::now();
        let timeout_duration = Duration::from_secs(10);
        self.pending_pings.retain(|_, &mut timestamp| {
            now.duration_since(timestamp).unwrap_or(Duration::from_secs(0)) < timeout_duration
        });
        
        if self.is_monitoring {
            let duration = now.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
            let current_second = duration.as_secs();
            let current_5sec_boundary = (current_second / 5) * 5;
            
            let should_ping = match self.last_ping_second {
                Some(last) => current_5sec_boundary > last,
                None => current_second % 5 == 0,
            };

            if should_ping {
                let circle_index = Self::get_circle_index_for_time(now);
                
                // Only start a new ping if we're not already pinging this circle
                if !self.pending_pings.contains_key(&circle_index) {
                    // Initialize channel if needed
                    if self.ping_receiver.is_none() {
                        let (sender, receiver) = mpsc::channel();
                        self.ping_receiver = Some(receiver);
                        self.ping_sender = Some(sender);
                    }
                    
                    // Start the ping using the existing sender
                    if let Some(sender) = &self.ping_sender {
                        // Resolve target with DNS caching
                        let target = self.target.clone();
                        let sender_clone = sender.clone();
                        let cache_entry = self.dns_cache.get(&target);
                        
                        // Check if we have a valid cached IP
                        if let Some(entry) = cache_entry {
                            if !entry.is_expired() {
                                // Use cached IP
                                self.start_async_ping_with_ip(entry.ip_address, circle_index, sender_clone);
                                self.pending_pings.insert(circle_index, now);
                                self.last_ping_second = Some(current_5sec_boundary);
                            } else {
                                // Cache expired, remove it and resolve again
                                self.dns_cache.remove(&target);
                                self.resolve_and_ping_async(target, circle_index, sender_clone);
                                self.pending_pings.insert(circle_index, now);
                                self.last_ping_second = Some(current_5sec_boundary);
                            }
                        } else {
                            // No cache entry, need to resolve
                            self.resolve_and_ping_async(target, circle_index, sender_clone);
                            self.pending_pings.insert(circle_index, now);
                            self.last_ping_second = Some(current_5sec_boundary);
                        }
                    }
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ping Monitor");
            
            ui.horizontal(|ui| {
                ui.label("Target (IP or hostname):");
                ui.add_enabled(!self.is_monitoring, egui::TextEdit::singleline(&mut self.target));
            });
            
            ui.label("Time Thresholds:");
            ui.horizontal(|ui| {
                ui.label("Green < ");
                ui.add(egui::DragValue::new(&mut self.green_threshold).range(1..=1000));
                ui.label("[ms]");
                ui.label("≤ Yellow <");
                ui.add(egui::DragValue::new(&mut self.yellow_threshold).range(1..=2000));
                ui.label("[ms]");
                ui.label("≤ Orange");
            });
            
            ui.horizontal(|ui| {
                if ui.button(if self.is_monitoring { "Stop" } else { "Start" }).clicked() {
                    self.is_monitoring = !self.is_monitoring;
                    if self.is_monitoring {
                        self.last_ping_second = None;
                    }
                }
            });
            
            ui.separator();
            
            ui.label(format!("Success Rate: {:.1}%", 100.0 - self.ping_statistics.loss_rate));
            ui.label(format!("Loss Rate: {:.1}%", self.ping_statistics.loss_rate));
            ui.label(format!("Mean Response Time: {:.1}ms", self.ping_statistics.mean_response_time));
            ui.label(format!("Last Response Time: {}", 
                match self.last_response_time {
                    Some(time) => format!("{time:.1}ms"),
                    None => "N/A".to_string(),
                }
            ));

            ui.separator();
            
            let clock_height = 240.0;
            
            ui.allocate_ui(Vec2::new(ui.available_width(), clock_height), |ui| {
                self.draw_clock_face(ui);
            });

            
        });
        
        if previous_target != self.target || previous_green != self.green_threshold || previous_yellow != self.yellow_threshold {
            self.save_config();
        }
        
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}