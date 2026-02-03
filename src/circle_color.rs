use egui::Color32;

const AGE_THRESHOLD_FULL_COLOR: f64 = 35.0;
const AGE_THRESHOLD_GRAY: f64 = 55.0;

#[derive(Debug, Clone, Copy)]
pub enum CircleColor {
    Gray,
    Green,
    Yellow,
    Orange,
    Red,
}

impl CircleColor {
    pub fn to_color32(self) -> Color32 {
        match self {
            CircleColor::Gray => Color32::GRAY,
            CircleColor::Green => Color32::GREEN,
            CircleColor::Yellow => Color32::YELLOW,
            CircleColor::Orange => Color32::from_rgb(255, 165, 0),
            CircleColor::Red => Color32::RED,
        }
    }
    
    pub fn to_color32_with_age(self, elapsed_seconds: f64) -> Color32 {
        if elapsed_seconds >= AGE_THRESHOLD_GRAY {
            return Color32::GRAY;
        }
        
        let base_color = self.to_color32();
        
        if elapsed_seconds <= AGE_THRESHOLD_FULL_COLOR {
            return base_color;
        }
        
        // Fade from full color to gray over the age threshold range
        let fade_range = AGE_THRESHOLD_GRAY - AGE_THRESHOLD_FULL_COLOR;
        let fade_factor = 1.0 - (elapsed_seconds - AGE_THRESHOLD_FULL_COLOR) / fade_range;
        let fade_factor = fade_factor.clamp(0.0, 1.0) as f32;
        
        Self::blend_colors(base_color, Color32::GRAY, fade_factor)
    }

    fn blend_colors(color1: Color32, color2: Color32, factor: f32) -> Color32 {
        Color32::from_rgb(
            (color1.r() as f32 * factor + color2.r() as f32 * (1.0 - factor)) as u8,
            (color1.g() as f32 * factor + color2.g() as f32 * (1.0 - factor)) as u8,
            (color1.b() as f32 * factor + color2.b() as f32 * (1.0 - factor)) as u8,
        )
    }

    pub fn from_ping_response(response_time_ms: Option<f64>, green_threshold: u64, yellow_threshold: u64) -> Self {
        match response_time_ms {
            Some(time) if time < green_threshold as f64 => CircleColor::Green,
            Some(time) if time < yellow_threshold as f64 => CircleColor::Yellow,
            Some(_) => CircleColor::Orange,
            None => CircleColor::Red,
        }
    }
}
