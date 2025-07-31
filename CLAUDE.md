# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MC-Link is a Rust-based Minecraft server management tool that compares and synchronizes mod installations between client and server instances. The project uses a modular workspace architecture with 5 main crates:

- **mc-link-core**: Core abstractions and server connection traits
- **mc-link-config**: Configuration management system with TOML-based server definitions
- **mc-link-connector**: Connection implementations (FTP, local filesystem)
- **mc-link-compat**: Mod compatibility checking and JAR file analysis  
- **mc-link-manager**: High-level management operations and comparison logic

## Architecture

The system follows a layered architecture:

1. **Config Layer** (`mc-link-config`): Manages server configurations stored in TOML files in the user's config directory
2. **Connection Layer** (`mc-link-connector`): Abstracts different ways to connect to servers (local, FTP)
3. **Core Layer** (`mc-link-core`): Defines traits like `ServerConnector` and core types like `ModInfo`
4. **Compatibility Layer** (`mc-link-compat`): Analyzes JAR files and checks mod compatibility between client/server
5. **Management Layer** (`mc-link-manager`): Orchestrates scanning, comparison, and sync operations

The main entry point creates a `MinecraftManager` from server configurations and performs comparison operations between client and server instances.

## Development Commands

### Building
```bash
cargo build
```

### Running
```bash
cargo run
```

### Testing
```bash
cargo test
```

### Linting
```bash
cargo clippy
```

### Formatting
```bash
cargo fmt
```

## Key Configuration

- Uses Rust 2024 edition across all crates
- Configuration files stored in OS-specific config directory via `directories` crate
- Logging via `tracing` with file output to config/logs directory
- Uses workspace dependencies for common crates (tokio, serde, thiserror, etc.)
- Custom registry "gitea" used for `tosic-utils` dependency

## Important Implementation Details

- The global `CONFIG_MANAGER` is lazily initialized and handles all configuration operations
- `MinecraftManager::from_config()` creates managers with appropriate connectors based on connection type
- Server scanning populates a `MinecraftStructure` containing mod information
- Comparison between managers generates a `SyncPlan` with specific actions
- All async operations use tokio runtime
- Error handling uses `miette` for user-friendly error reporting