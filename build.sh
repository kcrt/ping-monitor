#!/bin/bash
set -e

usage() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  --windows    Build Windows executable bundle (.exe)"
    echo "  --macos      Build macOS application bundle (.app)"
    echo "  --install    Build and install macOS app to /Applications (macOS only)"
    echo "  --all        Build for all platforms"
    echo "  --auto       Auto-detect platform and build (default for CI/CD)"
    echo "  --help       Display this help message"
    exit 1
}

if ! command -v cargo-bundle &> /dev/null; then
    echo "cargo-bundle is not installed. Installing now..."
    cargo install cargo-bundle
fi

# Auto-detect platform if no arguments provided
if [ $# -eq 0 ]; then
    echo "No arguments provided. Auto-detecting platform..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        set -- --macos
    elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
        set -- --windows
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux doesn't use cargo-bundle for releases, just regular build
        echo "Linux detected. Building with cargo build --release..."
        cargo build --release
        echo "Build completed. Binary available at target/release/ping-monitor"
        exit 0
    else
        usage
    fi
fi

BUILD_WINDOWS=false
BUILD_MACOS=false
INSTALL_MACOS=false

while [ $# -gt 0 ]; do
    case "$1" in
        --windows)
            BUILD_WINDOWS=true
            ;;
        --macos)
            BUILD_MACOS=true
            ;;
        --install)
            if [[ "$OSTYPE" != "darwin"* ]]; then
                echo "Error: --install option is only supported on macOS"
                exit 1
            fi
            BUILD_MACOS=true
            INSTALL_MACOS=true
            ;;
        --all)
            BUILD_WINDOWS=true
            BUILD_MACOS=true
            ;;
        --auto)
            # Auto mode already handled above
            ;;
        --help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
    shift
done

if [ "$BUILD_WINDOWS" = true ]; then
    echo "Building Windows executable bundle..."
    if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
        cargo bundle --release --format msi
    else
        echo "Cross-compiling for Windows..."
        echo "Note: This requires the appropriate cross-compilation toolchain."
        echo "If this fails, please build on a Windows machine or use a Windows VM."
        cargo bundle --release --target x86_64-pc-windows-gnu --format msi
    fi
    echo "Windows build completed. Check the 'target/release/bundle/msi/' directory."
fi

if [ "$BUILD_MACOS" = true ]; then
    echo "Building macOS application bundle..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        cargo bundle --release --format osx
    else
        echo "Cross-compiling for macOS..."
        echo "Note: This requires the appropriate cross-compilation toolchain."
        echo "If this fails, please build on a macOS machine or use a macOS VM."
        cargo bundle --release --target x86_64-apple-darwin --format osx
    fi
    echo "macOS build completed. Check the 'target/release/bundle/osx/' directory."
    
    if [ "$INSTALL_MACOS" = true ]; then
        echo "Installing PingMonitor.app to /Applications..."
        APP_PATH="target/release/bundle/osx/PingMonitor.app"
        if [ -d "$APP_PATH" ]; then
            # Remove existing installation if it exists
            if [ -d "/Applications/PingMonitor.app" ]; then
                echo "Removing existing installation..."
                rm -rf "/Applications/PingMonitor.app"
            fi
            
            # Copy the new app to /Applications
            cp -r "$APP_PATH" "/Applications/"
            echo "Successfully installed PingMonitor.app to /Applications"
        else
            echo "Error: Built application not found at $APP_PATH"
            exit 1
        fi
    fi
fi

echo "Build process completed!"