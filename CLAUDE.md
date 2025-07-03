# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PingMonitor is a GUI network monitoring application built with Rust and egui that tracks network connectivity using ping commands. The application visualizes ping results in a clock-face format with 12 circles representing 5-second intervals over a 60-second cycle.

## Build Commands

- **Build**: `cargo build`
- **Run**: `cargo run`
- **Build for release**: `cargo build --release`

## Architecture

### Core Components

**PingMonitorApp** (src/lib.rs): Main application state containing:
- `circles: [CircleColor; 12]` - Visual state of the 12 clock positions
- `circle_timestamps: [Option<SystemTime>; 12]` - Tracks when each circle was last updated for color aging
- `ping_results: VecDeque<PingResult>` - Rolling buffer of recent ping results (max 60)
- `ping_statistics: PingStatistics` - Computed statistics (success rate, mean response time, etc.)

**Clock Face Visualization**: 
- 12 circles positioned at clock positions (0, 5, 10, 15... 55 seconds)
- Each circle represents a 5-second interval within the current minute
- Colors indicate ping status: Gray (no data/expired), Green (<100ms), Yellow (100-200ms), Orange (>200ms), Red (failed)
- Red second hand shows current position within the 60-second cycle

**Ping Logic**:
- Pings every 5 seconds when monitoring is active
- Cross-platform ping command execution (Windows vs Unix)
- Response time parsing from platform-specific ping output
- Circle position calculated using `(seconds % 60) / 5` for current timestamp

### Key Behaviors

- Colors age over time: full color 0-35 seconds, gradual fade to gray 35-55 seconds, completely gray after 55 seconds
- Red border appears on circles during active ping operations
- Pings occur at real-world 5-second boundaries (0, 5, 10, 15... seconds past each minute)
- Second hand moves smoothly using millisecond precision
- Statistics are recalculated after each ping
- UI refreshes at 20 FPS (50ms intervals) for smooth animation
- Application window is always-on-top and non-resizable (450x600)

### Dependencies

- **egui/eframe**: GUI framework for cross-platform desktop application
- **tokio**: Async runtime (full features enabled)
- **chrono**: Date and time handling
- **serde**: Serialization support

## File Structure

- `src/main.rs`: Application entry point and window configuration
- `src/lib.rs`: Core application logic, ping implementation, and UI rendering

## Configuration

The application saves the ping target persistently in a JSON configuration file:
- **macOS**: `~/Library/Application Support/PingMonitor/config.json`
- **Windows**: `%APPDATA%/PingMonitor/config.json` 
- **Linux**: `~/.config/PingMonitor/config.json`

The target is automatically saved when changed and loaded on startup.