mod config;
mod ping;
mod dns_cache;
mod ping_executor;
mod circle_color;

use std::collections::{VecDeque, HashMap};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use eframe::egui;
use egui::{Color32, Vec2, Pos2, Stroke};
use std::sync::mpsc;

use config::AppConfig;
use ping::{PingResult, PingStatistics};
use dns_cache::{DnsCache, DnsCacheEntry};
use ping_executor::PingExecutor;
use circle_color::CircleColor;

// Constants
const PING_INTERVAL_SECS: u64 = 5;
const MAX_PING_RESULTS: usize = 60;
const STATISTICS_WINDOW_SECS: u64 = 60;
const PENDING_PING_TIMEOUT_SECS: u64 = 10;
const DNS_CACHE_TTL_SECS: u64 = 300;
const NUM_CIRCLES: usize = 12;

pub struct PingMonitorApp {
    pub target: String,
    pub is_monitoring: bool,
    pub ping_results: VecDeque<PingResult>,
    pub circles: [CircleColor; NUM_CIRCLES],
    pub circle_timestamps: [Option<SystemTime>; NUM_CIRCLES],
    pub last_ping_second: Option<u64>,
    pub ping_statistics: PingStatistics,
    pub ping_receiver: Option<mpsc::Receiver<PingResult>>,
    pub ping_sender: Option<mpsc::Sender<PingResult>>,
    pub pending_pings: HashMap<usize, SystemTime>,
    pub dns_cache: DnsCache,
    pub green_threshold: u64,
    pub yellow_threshold: u64,
    pub last_response_time: Option<f64>,
}



impl Default for PingMonitorApp {
    fn default() -> Self {
        Self {
            target: "8.8.8.8".to_string(),
            is_monitoring: false,
            ping_results: VecDeque::new(),
            circles: [CircleColor::Gray; NUM_CIRCLES],
            circle_timestamps: [None; NUM_CIRCLES],
            last_ping_second: None,
            ping_statistics: PingStatistics::default(),
            ping_receiver: None,
            ping_sender: None,
            pending_pings: HashMap::new(),
            dns_cache: DnsCache::new(),
            green_threshold: 100,
            yellow_threshold: 200,
            last_response_time: None,
        }
    }
}

impl PingMonitorApp {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            target: config.target,
            is_monitoring: false,
            ping_results: VecDeque::new(),
            circles: [CircleColor::Gray; NUM_CIRCLES],
            circle_timestamps: [None; NUM_CIRCLES],
            last_ping_second: None,
            ping_statistics: PingStatistics::default(),
            ping_receiver: None,
            ping_sender: None,
            pending_pings: HashMap::new(),
            dns_cache: DnsCache::new(),
            green_threshold: config.green_threshold,
            yellow_threshold: config.yellow_threshold,
            last_response_time: None,
        }
    }

    fn save_config(&self) {
        let config = AppConfig {
            target: self.target.clone(),
            green_threshold: self.green_threshold,
            yellow_threshold: self.yellow_threshold,
        };

        if let Err(e) = config.save() {
            eprintln!("Failed to save config: {e}");
        }
    }

    fn get_circle_color(&self, ping_result: &PingResult) -> CircleColor {
        if !ping_result.success {
            return CircleColor::Red;
        }
        
        CircleColor::from_ping_response(
            ping_result.response_time,
            self.green_threshold,
            self.yellow_threshold
        )
    }

    fn update_statistics(&mut self) {
        let now = SystemTime::now();
        let cutoff_time = now - Duration::from_secs(STATISTICS_WINDOW_SECS);
        
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
        
        self.draw_circles(center, radius, circle_radius, painter, ui);
        self.draw_second_hand(center, radius, painter);
    }

    fn draw_circles(&self, center: Pos2, radius: f32, circle_radius: f32, painter: &egui::Painter, ui: &egui::Ui) {
        for i in 0..NUM_CIRCLES {
            let angle = (i as f32 * 30.0 - 90.0) * std::f32::consts::PI / 180.0;
            let pos = Self::place_in_circle(center, radius, angle);
            
            let color = self.get_circle_color_with_age(i);
            painter.circle_filled(pos, circle_radius, color);
            
            let stroke_color = if self.pending_pings.contains_key(&i) {
                Color32::RED
            } else {
                Color32::BLACK
            };
            painter.circle_stroke(pos, circle_radius, Stroke::new(2.0, stroke_color));
            
            self.draw_circle_label(center, radius, angle, i, painter, ui);
        }
    }

    fn get_circle_color_with_age(&self, circle_index: usize) -> Color32 {
        if let Some(timestamp) = self.circle_timestamps[circle_index] {
            if let Ok(elapsed) = SystemTime::now().duration_since(timestamp) {
                let elapsed_seconds = elapsed.as_secs_f64();
                return self.circles[circle_index].to_color32_with_age(elapsed_seconds);
            }
        }
        self.circles[circle_index].to_color32()
    }

    fn draw_circle_label(&self, center: Pos2, radius: f32, angle: f32, index: usize, painter: &egui::Painter, ui: &egui::Ui) {
        let text = format!("{}", index * 5);
        let text_pos = Self::place_in_circle(center, radius - 25.0, angle);
        let font = egui::FontId::monospace(12.0);
        painter.text(text_pos, egui::Align2::CENTER_CENTER, text, font, ui.visuals().text_color());
    }

    fn draw_second_hand(&self, center: Pos2, radius: f32, painter: &egui::Painter) {
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

    fn place_in_circle(center: Pos2, radius: f32, angle: f32) -> Pos2 {
        Pos2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        )
    }
}

impl eframe::App for PingMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let previous_target = self.target.clone();
        let previous_green = self.green_threshold;
        let previous_yellow = self.yellow_threshold;
        
        // Process incoming ping results
        self.process_ping_results();
        
        // Clean up old pending pings
        self.cleanup_pending_pings();
        
        // Handle periodic pinging
        if self.is_monitoring {
            self.handle_periodic_ping();
        }

        // Render UI
        self.render_ui(ctx);
        
        // Save config if changed
        if previous_target != self.target || previous_green != self.green_threshold || previous_yellow != self.yellow_threshold {
            self.save_config();
        }
        
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl PingMonitorApp {
    fn process_ping_results(&mut self) {
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
            
            self.last_response_time = ping_result.response_time;
            
            // Update DNS cache if we have resolution info
            if let Some((hostname, ip)) = &ping_result.resolved_ip {
                if hostname != &ip.to_string() {
                    self.dns_cache.insert(hostname.clone(), DnsCacheEntry::new(*ip, DNS_CACHE_TTL_SECS));
                }
            }
            
            self.ping_results.push_back(ping_result);
            
            if self.ping_results.len() > MAX_PING_RESULTS {
                self.ping_results.pop_front();
            }
            
            self.update_statistics();
            self.pending_pings.remove(&circle_index);
        }
    }

    fn cleanup_pending_pings(&mut self) {
        let now = SystemTime::now();
        let timeout_duration = Duration::from_secs(PENDING_PING_TIMEOUT_SECS);
        self.pending_pings.retain(|_, &mut timestamp| {
            now.duration_since(timestamp).unwrap_or(Duration::from_secs(0)) < timeout_duration
        });
    }

    fn handle_periodic_ping(&mut self) {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
        let current_second = duration.as_secs();
        let current_5sec_boundary = (current_second / PING_INTERVAL_SECS) * PING_INTERVAL_SECS;
        
        let should_ping = match self.last_ping_second {
            Some(last) => current_5sec_boundary > last,
            None => current_second % PING_INTERVAL_SECS == 0,
        };

        if should_ping {
            self.initiate_ping(now, current_5sec_boundary);
        }
    }

    fn initiate_ping(&mut self, now: SystemTime, current_5sec_boundary: u64) {
        let circle_index = Self::get_circle_index_for_time(now);
        
        // Only start a new ping if we're not already pinging this circle
        if self.pending_pings.contains_key(&circle_index) {
            return;
        }

        // Initialize channel if needed
        if self.ping_receiver.is_none() {
            let (sender, receiver) = mpsc::channel();
            self.ping_receiver = Some(receiver);
            self.ping_sender = Some(sender);
        }
        
        if let Some(sender) = &self.ping_sender {
            let target = self.target.clone();
            let sender_clone = sender.clone();
            
            // Check for valid cached IP
            if let Some(cached_ip) = self.dns_cache.get_valid_ip(&target) {
                PingExecutor::ping_with_ip(cached_ip, sender_clone);
            } else {
                // Clean expired cache and resolve
                self.dns_cache.clean_expired(&target);
                PingExecutor::resolve_and_ping(target, sender_clone);
            }
            
            self.pending_pings.insert(circle_index, now);
            self.last_ping_second = Some(current_5sec_boundary);
        }
    }

    fn render_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ping Monitor");
            
            self.render_target_input(ui);
            self.render_threshold_controls(ui);
            self.render_control_buttons(ui);
            
            ui.separator();
            
            self.render_statistics(ui);
            
            ui.separator();
            
            let clock_height = 240.0;
            ui.allocate_ui(Vec2::new(ui.available_width(), clock_height), |ui| {
                self.draw_clock_face(ui);
            });
        });
    }

    fn render_target_input(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Target (IP or hostname):");
            ui.add_enabled(!self.is_monitoring, egui::TextEdit::singleline(&mut self.target));
        });
    }

    fn render_threshold_controls(&mut self, ui: &mut egui::Ui) {
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
    }

    fn render_control_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button(if self.is_monitoring { "Stop" } else { "Start" }).clicked() {
                self.is_monitoring = !self.is_monitoring;
                if self.is_monitoring {
                    self.last_ping_second = None;
                }
            }
        });
    }

    fn render_statistics(&self, ui: &mut egui::Ui) {
        ui.label(format!("Success Rate: {:.1}%", 100.0 - self.ping_statistics.loss_rate));
        ui.label(format!("Loss Rate: {:.1}%", self.ping_statistics.loss_rate));
        ui.label(format!("Mean Response Time: {:.1}ms", self.ping_statistics.mean_response_time));
        ui.label(format!("Last Response Time: {}", 
            match self.last_response_time {
                Some(time) => format!("{time:.1}ms"),
                None => "N/A".to_string(),
            }
        ));
    }
}