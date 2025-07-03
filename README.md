# PingMonitor

A cross-platform GUI network monitoring application built with Rust and egui that tracks network connectivity using ping commands. The application visualizes ping results in an intuitive clock-face format with color-coded status indicators.

## Features

- **Clock-Face Visualization**: 12 circles positioned at clock positions representing 5-second intervals over a 60-second cycle
- **Color-Coded Status**: 
  - 🟢 Green: Response time < 100ms
  - 🟡 Yellow: Response time 100-200ms
  - 🟠 Orange: Response time > 200ms
  - 🔴 Red: Failed ping
  - ⚫ Gray: No data or expired (after 55 seconds)
- **DNS Caching**: Intelligent DNS resolution caching with 5-minute TTL to reduce network overhead
- **Real-time Statistics**: Success rate, loss rate, and mean response time
- **Persistent Configuration**: Automatically saves and loads ping target
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Always-on-Top Window**: Stays visible while working with other applications

## Installation

### Prerequisites
- Rust 1.70+ (2024 edition)
- Cargo package manager

### Building from Source
```bash
git clone <repository-url>
cd ping-monitor
cargo build --release
```

### Running
```bash
cargo run
```

## Usage

1. **Set Target**: Enter the IP address or hostname you want to monitor (default: 8.8.8.8)
2. **Start Monitoring**: Click the "Start" button to begin ping monitoring
3. **View Results**: 
   - The clock face shows ping results for the last 60 seconds
   - Each circle represents a 5-second interval
   - Colors fade over time and turn gray after 55 seconds
   - The red second hand shows the current position in the 60-second cycle
4. **Statistics**: View real-time statistics including success rate, loss rate, and mean response time

## Technical Details

### Architecture
- **Frontend**: egui/eframe for cross-platform GUI
- **Ping Logic**: surge-ping library for ICMP ping functionality
- **DNS Resolution**: Built-in DNS caching with configurable TTL
- **Data Management**: Rolling buffer of 60 ping results
- **Configuration**: JSON-based persistent storage

### Configuration Storage
- **macOS**: `~/Library/Application Support/PingMonitor/config.json`
- **Windows**: `%APPDATA%/PingMonitor/config.json`
- **Linux**: `~/.config/PingMonitor/config.json`

### Key Behaviors
- Pings occur every 5 seconds at real-world boundaries (0, 5, 10, 15... seconds)
- DNS resolution is cached for 5 minutes to minimize network overhead
- Circle colors age over time with gradual fading
- Statistics are calculated from the last 60 seconds of data
- UI refreshes at 20 FPS for smooth animations
- Window is always-on-top and non-resizable (450x600)

## Dependencies

- **egui/eframe**: GUI framework
- **surge-ping**: ICMP ping implementation
- **chrono**: Date and time handling
- **serde/serde_json**: Configuration serialization
- **dirs**: Platform-specific directory paths
- **tokio**: Async runtime (full features)

## Build Commands

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run application
cargo run
```

## License

Copyright (c) kcrt 2025. All rights reserved.

## Contributing

This project is built with defensive security practices in mind. Please ensure any contributions follow secure coding practices and do not introduce vulnerabilities.