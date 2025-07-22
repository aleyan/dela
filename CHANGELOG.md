# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [0.6.0] - 2025-07-22

### Added

- Added support for travis ci.
- Added support for docker-compose.
- Added support for cmake.
- Added color to the `dela list` output.

### Changed

- Allow dialog is now a tui with 5 selectable options.
- Update the `dela --help` command to filter out internal commands.

## [0.5.0] - 2025-05-17

### Added

- Added renaming of shadowed tasks.

### Changed

- Added fallback parsing for `make` via regex.
- Increased robustness of `task` parser.
- New output layout for `dela list`.

## [0.4.0] - 2025-03-27

### Added

- Added `maven` and `gradle` support.
- Added `Github Actions` support via `act`.

## [0.3.0] - 2025-02-23

### Added

- Indicate in `dela list` when a runner is missing.

### Changed

- For package.json, correctly pick the runner based on the lock file.

### Deprecated

### Removed

### Fixed

### Security

## [0.2.0] - 2025-02-16
- Initial release