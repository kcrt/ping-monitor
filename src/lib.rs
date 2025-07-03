use std::collections::VecDeque;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::fs;
use eframe::egui;
use egui::{Color32, Vec2, Pos2, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PingResult {
    pub timestamp: SystemTime,
    pub response_time: Option<u64>,
    pub success: bool,
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
    pub active_ping_circle: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct PingStatistics {
    pub total_pings: u64,
    pub successful_pings: u64,
    pub failed_pings: u64,
    pub total_response_time: u64,
    pub loss_rate: f64,
    pub mean_response_time: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    target: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            target: "8.8.8.8".to_string(),
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
            active_ping_circle: None,
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
            active_ping_circle: None,
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
                                Err(e) => eprintln!("Failed to parse config: {}", e),
                            }
                        }
                        Err(e) => eprintln!("Failed to read config file: {}", e),
                    }
                }
            }
            Err(e) => eprintln!("Failed to get config path: {}", e),
        }
        AppConfig::default()
    }

    fn save_config(&self) {
        let config = AppConfig {
            target: self.target.clone(),
        };

        match Self::get_config_path() {
            Ok(path) => {
                match serde_json::to_string_pretty(&config) {
                    Ok(content) => {
                        if let Err(e) = fs::write(&path, content) {
                            eprintln!("Failed to save config: {}", e);
                        }
                    }
                    Err(e) => eprintln!("Failed to serialize config: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to get config path: {}", e),
        }
    }

    fn perform_ping(&self, target: &str) -> PingResult {
        let timestamp = SystemTime::now();
        
        let output = if cfg!(target_os = "windows") {
            Command::new("ping")
                .args(&["-n", "1", "-w", "5000", target])
                .output()
        } else {
            Command::new("ping")
                .args(&["-c", "1", "-W", "5000", target])
                .output()
        };

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if output.status.success() {
                    let response_time = Self::parse_ping_time(&stdout);
                    PingResult {
                        timestamp,
                        response_time,
                        success: true,
                    }
                } else {
                    PingResult {
                        timestamp,
                        response_time: None,
                        success: false,
                    }
                }
            }
            Err(_) => PingResult {
                timestamp,
                response_time: None,
                success: false,
            },
        }
    }

    fn parse_ping_time(output: &str) -> Option<u64> {
        if cfg!(target_os = "windows") {
            if let Some(line) = output.lines().find(|line| line.contains("time=") || line.contains("time<")) {
                if let Some(start) = line.find("time") {
                    let time_part = &line[start..];
                    if let Some(eq_pos) = time_part.find('=') {
                        let after_eq = &time_part[eq_pos + 1..];
                        if let Some(ms_pos) = after_eq.find("ms") {
                            let time_str = &after_eq[..ms_pos];
                            return time_str.trim().parse::<u64>().ok();
                        }
                    } else if time_part.contains("time<") {
                        return Some(1);
                    }
                }
            }
        } else {
            if let Some(line) = output.lines().find(|line| line.contains("time=")) {
                if let Some(start) = line.find("time=") {
                    let time_part = &line[start + 5..];
                    if let Some(space_pos) = time_part.find(' ') {
                        let time_str = &time_part[..space_pos];
                        return time_str.parse::<f64>().ok().map(|t| t as u64);
                    }
                }
            }
        }
        None
    }

    fn get_circle_color(ping_result: &PingResult) -> CircleColor {
        if !ping_result.success {
            return CircleColor::Red;
        }
        
        match ping_result.response_time {
            Some(time) => {
                if time < 100 {
                    CircleColor::Green
                } else if time < 200 {
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
        
        let total_response_time: u64 = recent_results
            .iter()
            .filter_map(|r| r.response_time)
            .sum();
        
        self.ping_statistics = PingStatistics {
            total_pings: total,
            successful_pings: successful,
            failed_pings: failed,
            total_response_time,
            loss_rate: if total > 0 { (failed as f64 / total as f64) * 100.0 } else { 0.0 },
            mean_response_time: if successful > 0 { total_response_time as f64 / successful as f64 } else { 0.0 },
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
        
        for i in 0..12 {
            let angle = (i as f32 * 30.0 - 90.0) * std::f32::consts::PI / 180.0;
            let x = center.x + radius * angle.cos();
            let y = center.y + radius * angle.sin();
            let pos = Pos2::new(x, y);
            
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
            
            let stroke_color = if self.active_ping_circle == Some(i) {
                Color32::RED
            } else {
                Color32::BLACK
            };
            painter.circle_stroke(pos, circle_radius, Stroke::new(2.0, stroke_color));
            
            let text = format!("{}", i * 5);
            let text_pos = Pos2::new(x, y + circle_radius + 15.0);
            painter.text(text_pos, egui::Align2::CENTER_CENTER, text, egui::FontId::default(), Color32::BLACK);
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
        if self.is_monitoring {
            let now = SystemTime::now();
            let duration = now.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
            let current_second = duration.as_secs();
            let current_5sec_boundary = (current_second / 5) * 5;
            
            let should_ping = match self.last_ping_second {
                Some(last) => current_5sec_boundary > last,
                None => current_second % 5 == 0,
            };

            if should_ping {
                let circle_index = Self::get_circle_index_for_time(now);
                self.active_ping_circle = Some(circle_index);
                
                let ping_result = self.perform_ping(&self.target);
                self.circles[circle_index] = Self::get_circle_color(&ping_result);
                self.circle_timestamps[circle_index] = Some(ping_result.timestamp);
                
                self.ping_results.push_back(ping_result);
                
                if self.ping_results.len() > 60 {
                    self.ping_results.pop_front();
                }
                
                self.update_statistics();
                self.last_ping_second = Some(current_5sec_boundary);
                self.active_ping_circle = None;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ping Monitor");
            
            ui.horizontal(|ui| {
                ui.label("Target:");
                ui.text_edit_singleline(&mut self.target);
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
            
            ui.label(format!("Total Pings: {}", self.ping_statistics.total_pings));
            ui.label(format!("Success Rate: {:.1}%", 100.0 - self.ping_statistics.loss_rate));
            ui.label(format!("Loss Rate: {:.1}%", self.ping_statistics.loss_rate));
            ui.label(format!("Mean Response Time: {:.1}ms", self.ping_statistics.mean_response_time));

            ui.separator();
            
            let clock_height = 240.0;
            
            ui.allocate_ui(Vec2::new(ui.available_width(), clock_height), |ui| {
                self.draw_clock_face(ui);
            });

            
        });
        
        if previous_target != self.target {
            self.save_config();
        }
        
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}