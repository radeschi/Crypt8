# Changelog

All notable changes to Crypt8 are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Renamed the product from orangeEncrypt to Crypt8. The app identifier and the updater signing key stay the same.
- Started `CHANGELOG.md` and recorded the changes already committed, from the first public release through 1.1.0.

## [1.1.0] - 2026-09-25

### Added

- Encrypt and decrypt a short message with the same local OpenPGP setup used for files. The plaintext limit is 2,000 Unicode characters. The result is ASCII-armored OpenPGP text, available in every supported language.
- Release asset names now include the operating system and architecture. `latest.json` keeps its name.

### Fixed

- Restored the system Edit menu so text fields receive the native copy, paste, cut, select-all, undo, and redo shortcuts.
- The message box fills the space left in the window and scrolls inside itself.

## [1.0.8] - 2026-09-25

### Changed

- Bumped the version to 1.0.8.
- Stopped tracking the local version-bump script.

## [1.0.6] - 2026-09-25

### Changed

- Rewrote the English and Portuguese READMEs for the public launch and added the demo GIF.
- Bumped the version to 1.0.6.

## [1.0.5] - 2026-09-24

### Changed

- Bumped the version to 1.0.5.

## [1.0.4] - 2026-09-24

### Added

- Signed automatic updates from GitHub Releases, checked on startup without interrupting encryption, plus a manual check on About.

## [1.0.3] - 2026-09-24

### Added

- First public release: local OpenPGP encryption for files and folders, with no account and no upload.
- GitHub Actions builds for macOS (Apple Silicon and Intel), Linux (x64 and ARM), and Windows (x64 and ARM).
- GPL-3.0 license.
- English README as the default, with the Portuguese README kept as `README-pt_BR.md`.

### Changed

- Removed the unused password hint from the encrypt form. It was never stored in the `.gpg` file.

### Fixed

- macOS builds use ad-hoc signing when no Apple Developer ID is configured, so the app is no longer reported as damaged.
- Linux builds install `xdg-utils`.
- Corrected the GitHub Actions build setup.

[1.1.0]: https://github.com/radeschi/Crypt8/releases/tag/v1.1.0
[1.0.8]: https://github.com/radeschi/Crypt8/releases/tag/v1.0.8
[1.0.6]: https://github.com/radeschi/Crypt8/releases/tag/v1.0.6
[1.0.5]: https://github.com/radeschi/Crypt8/releases/tag/v1.0.5
[1.0.4]: https://github.com/radeschi/Crypt8/releases/tag/v1.0.4
[1.0.3]: https://github.com/radeschi/Crypt8/releases/tag/v1.0.3
