# AxonHub Manager

[English](README.md) | [简体中文](README.zh-CN.md)

![Version](https://img.shields.io/badge/version-0.0.3-blue)
![Platform](https://img.shields.io/badge/platform-Windows-0078D4)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)
![Rust](https://img.shields.io/badge/Rust-2021-000000)
![React](https://img.shields.io/badge/React-19-61DAFB)

AxonHub Manager is a Windows desktop lifecycle manager for [AxonHub](https://github.com/looplj/axonhub). It installs and validates AxonHub releases, owns the AxonHub process, and provides a focused interface for configuration, health, logs, upgrades, and rollback.

AxonHub's built-in web console remains the place to manage channels, models, API keys, and request tracing.

## Features

- Fetch the latest release from `looplj/axonhub` and select the matching Windows x64 asset.
- Verify the release SHA-256 digest when the upstream release provides one.
- Keep the previous installed version for one-click rollback.
- Start AxonHub automatically with the desktop application.
- Stop AxonHub before the application exits so no managed process is left behind.
- Keep running in the system tray; the window close button hides the application instead of exiting.
- Monitor the AxonHub process and its `/health` endpoint.
- Edit `config.yml` with YAML checks and `axonhub config validate`.
- Restart the managed process automatically after configuration changes, upgrades, or rollback.
- View Manager events and AxonHub standard output in one log interface.
- Use dark or light themes with keyboard and reduced-motion accessibility support.

## Installation

Download the latest Windows x64 installer from [GitHub Releases](https://github.com/gcdd1993/Axonhub-Manager/releases).

The current installer is unsigned. Windows SmartScreen may display a warning until code signing is configured. Verify that downloads come from this repository before running them.

## Runtime Behavior

AxonHub Manager owns the AxonHub process lifecycle:

1. If AxonHub is installed, opening Manager starts it automatically.
2. Clicking the window close button hides Manager to the system tray while AxonHub keeps running.
3. Left-clicking the tray icon restores the window.
4. Choosing **Exit** from the tray stops AxonHub, waits for it to exit, and then closes Manager.

The application stores its managed files in the operating system application data directory:

```text
config.yml
axonhub.db
current.json
previous.json
versions/
logs/axonhub.log
```

## Requirements

- Windows 10 or later, x64
- Microsoft Edge WebView2 Runtime
- Network access to GitHub when installing or checking AxonHub releases

## Development

Install dependencies and start the Tauri application:

```powershell
npm ci
npm run tauri -- dev
```

Run the same checks used by CI:

```powershell
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Build the Windows NSIS installer:

```powershell
npm run tauri -- build --bundles nsis
```

The installer is written to `src-tauri/target/release/bundle/nsis/`.

## Releases

Pull requests and pushes to `master` run frontend and Rust verification. A tag matching `v*` builds the Windows x64 NSIS installer and publishes it to GitHub Releases. A manual workflow run creates a draft release.

Before tagging a release:

1. Update `VERSION`, `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json` to the same version.
2. Move the release notes from **Unreleased** into a dated section in `CHANGELOG.md`.
3. Commit the changes and push a matching tag, for example `v0.0.1`.

The workflow rejects inconsistent versions and tags.

## Technology

- Tauri 2
- Rust 2021
- React 19 and TypeScript
- Vite 6

## Upstream and Scope

- [AxonHub](https://github.com/looplj/axonhub) is the upstream service managed by this application.
- AxonHub Manager is an independent desktop companion and is not an official AxonHub distribution.
- The product boundary was informed by [CPA-Manager-native](https://github.com/gcdd1993/CPA-Manager-native).

## License

No license file is currently included. All rights remain with their respective owners unless a license is added.
