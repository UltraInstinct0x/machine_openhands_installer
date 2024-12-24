# OpenHands Container Manager

A Rust-based utility that automates the setup and management of OpenHands AI containers. This tool handles Docker installation, container deployment, and provides a WebSocket interface for communication.

## Features

- 🐳 Automatic Docker installation if not present
- 🚀 Automated container setup and management
- 🔄 WebSocket-based communication interface
- 🌐 HTTP API endpoint for command execution
- 💪 Cross-platform support (Linux, macOS, Windows)
- 🔁 Automatic retry mechanisms for reliability
- 🔌 Built-in server running on port 5000

## Prerequisites

- Rust toolchain
- Internet connection
- System privileges for Docker installation (if not already installed)

## Installation

```bash
git clone [repository-url]
cd openhands-container-manager
cargo build --release
```

## Usage
Run the executable:
The application will:

Check for Docker installation and install if missing
Pull and run the OpenHands container
Start a local server on port 5000
Send commands via HTTP POST:

## System Requirements
- Linux: Ubuntu/Debian-based systems
- macOS: 10.15 or later
- Windows: Windows 10 or later with WSL2 support
- Minimum 8GB RAM recommended
- Active internet connection

## Development
Built with:
- 🦀 Rust
- 🌊 Warp for HTTP server
- 🔌 Tokio WebSocket
- 🐳 Docker integration