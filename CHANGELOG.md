# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-23

### Added

- A drop-in replacement for the `VsixPublisher.exe` included in the Visual
  Studio SDK, running natively on Windows, Linux and macOS.
- The `publish`, `login`, `logout`, `deleteExtension`, `deletePublisher`,
  `help` and `version` commands. They take the same `-argument value` syntax
  and print the same `VsixPubNNNN` messages and exit codes as the original.
- Publishing `.vsix`, `.exe`/`.msi` and link payloads, with the same upload
  package and `extension.marketplacemanifest` as the original.
- Personal access tokens stored in the `~/.vsce` publisher file.
- Trusted publishing from GitHub Actions with `-oidc`. It uses the
  `FederatedToken` exchange that the Marketplace accepts and masks the
  short-lived credential in workflow logs.

[Unreleased]: https://github.com/christianhelle/vsixpublisher/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/christianhelle/vsixpublisher/releases/tag/v0.1.0
