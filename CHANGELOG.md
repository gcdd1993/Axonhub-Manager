# Changelog

All notable changes to AxonHub Manager are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.3] - 2026-08-12

### Added

- Added a manual AxonHub start action to the overview page.
- Added explicit migrate-and-overwrite or switch-without-migration choices for non-empty data directories.

### Fixed

- Repair stale executable paths automatically when an existing AxonHub data directory moves or its drive letter changes.
- Reload installed-version state correctly when switching to an existing data directory without migration.

## [0.0.2] - 2026-08-12

### Added

- Added system startup registration with an application settings toggle.
- Added selectable AxonHub data directories with validated migration and source preservation.

### Changed

- Start the Manager and managed AxonHub process without opening Windows console windows.
- Start the Manager hidden in the system tray and streamline the startup setting control.

## [0.0.1] - 2026-08-11

### Added

- Added the initial Tauri desktop manager for installing, validating, upgrading, and rolling back AxonHub releases.
- Added managed AxonHub process startup, health monitoring, configuration validation, and aggregated logs.
- Added automatic AxonHub startup with Manager and ordered shutdown before Manager exits.
- Added system tray background operation, window restoration, and explicit tray exit.
- Added dark and light themes and accessibility-focused interaction states.
- Added a Windows x64 NSIS installer build.
- Added English and Simplified Chinese documentation.
- Added GitHub Actions verification and tagged release publishing.

[Unreleased]: https://github.com/gcdd1993/Axonhub-Manager/compare/v0.0.3...HEAD
[0.0.3]: https://github.com/gcdd1993/Axonhub-Manager/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/gcdd1993/Axonhub-Manager/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/gcdd1993/Axonhub-Manager/releases/tag/v0.0.1
