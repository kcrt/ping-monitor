@echo off
REM Build script for PingMonitor application on Windows

echo Building PingMonitor for Windows...

REM Check if cargo-bundle is installed
cargo install --list | findstr "cargo-bundle" > nul
if %ERRORLEVEL% NEQ 0 (
    echo cargo-bundle is not installed. Installing now...
    cargo install cargo-bundle
)

REM Build the Windows executable bundle with release profile
echo Building Windows executable bundle...
cargo bundle --release --format msi

echo Build process completed!
echo Check the 'target\release\bundle\msi\' directory for the installer.