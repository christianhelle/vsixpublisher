# vsixpublisher

[![Crates.io](https://img.shields.io/crates/v/vsixpublisher.svg)](https://crates.io/crates/vsixpublisher)
[![CI](https://github.com/christianhelle/vsixpublisher/actions/workflows/ci.yml/badge.svg)](https://github.com/christianhelle/vsixpublisher/actions/workflows/ci.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A drop-in replacement for the `VsixPublisher.exe` included in the Visual Studio SDK, the tool that
publishes extensions to the [Visual Studio Marketplace](https://marketplace.visualstudio.com/).

The SDK's VsixPublisher is a .NET Framework app that comes with about 40 assemblies and only runs on
Windows with Visual Studio installed. vsixpublisher is a single native binary of under 1 MB that
runs on Windows, Linux and macOS. It takes the same commands, `-argument value` syntax and publish
manifest, prints the same messages and `VsixPubNNNN` error codes, returns the same exit codes, and
shares the same `~/.vsce` publisher store. Existing scripts keep working when you swap the path.

## Install

### macOS/Linux

```bash
curl -fsSL https://christianhelle.com/vsixpublisher/install | bash
```

This installs the latest release to `~/.local/bin`. To install somewhere else, or to pin a release:

```bash
curl -fsSL https://christianhelle.com/vsixpublisher/install | INSTALL_DIR="$HOME/bin" bash
curl -fsSL https://christianhelle.com/vsixpublisher/install | VERSION="0.1.0" bash
```

### Windows PowerShell

```powershell
irm https://christianhelle.com/vsixpublisher/install.ps1 | iex
```

This installs the latest release to `%LOCALAPPDATA%\Programs\vsixpublisher`, or to `~\.local\bin` or
`~\bin` if one of those already exists, and adds it to your user `PATH`. To install somewhere else, or
to pin a release:

```powershell
$install = irm https://christianhelle.com/vsixpublisher/install.ps1
& ([scriptblock]::Create($install)) -InstallDir "$env:USERPROFILE\bin"
& ([scriptblock]::Create($install)) -Version "0.1.0"
```

### Cargo

```bash
cargo install vsixpublisher
```

Prebuilt archives for x64 and ARM64 on Windows, Linux and macOS are on the
[releases page](https://github.com/christianhelle/vsixpublisher/releases).

## Replacing VsixPublisher.exe

Swap the SDK path for `vsixpublisher` and keep the arguments as they are:

```diff
- & "C:\Program Files\Microsoft Visual Studio\18\Enterprise\VSSDK\VisualStudioIntegration\Tools\Bin\VsixPublisher.exe" `
-     publish -payload My.vsix -publishManifest publishManifest.json -personalAccessToken $env:PAT
+ vsixpublisher publish -payload My.vsix -publishManifest publishManifest.json -personalAccessToken $env:PAT
```

Because it doesn't need Visual Studio, the publish step can also run on a Linux or macOS runner.

## Usage

```shell
vsixpublisher login -publisherName <name> -personalAccessToken <pat> [-overwrite]
vsixpublisher publish -payload <file.vsix|setup.exe|https://...> -publishManifest <publishManifest.json>
                      [-personalAccessToken <pat> | -oidc] [-ignoreWarnings "W1,W2"]
vsixpublisher logout -publisherName <name> [-ignoreMissingPublisher]
vsixpublisher deleteExtension -publisherName <name> -extensionName <name> [-personalAccessToken <pat>]
vsixpublisher deletePublisher -publisherName <name> -personalAccessToken <pat>
vsixpublisher help | version
```

Arguments can start with `-` or `/`, and their names are case-insensitive. Hidden arguments from the
original are supported too: `-marketplaceUrl <url>` and `-outputFilePath <file>` (writes the upload
package to disk).

### Publish manifest

```json
{
  "$schema": "http://json.schemastore.org/vsix-publish",
  "categories": ["coding", "build"],
  "identity": { "internalName": "my-extension" },
  "overview": "README.md",
  "priceCategory": "free",
  "publisher": "my-publisher",
  "private": false,
  "qna": true,
  "repo": "https://github.com/me/my-extension",
  "assetFiles": [{ "pathOnDisk": "images/screenshot.png", "targetPath": "images/screenshot.png" }]
}
```

For a `.vsix` payload the identity is read from `extension.vsixmanifest`, so the manifest only sets
`internalName`. For an `.exe`/`.msi` or a URL payload, `identity` must also give `displayName`,
`description`, `language`, `version` and `installTargets` (`[{ "sku": "Community", "version": "[17.0,18.0)" }]`).

### Trusted publishing (GitHub Actions)

`-oidc` swaps a GitHub Actions OIDC token for a Marketplace session token, so no PAT is needed:

```yaml
permissions:
  id-token: write
steps:
  - run: vsixpublisher publish -payload out/My.vsix -publishManifest publishManifest.json -oidc
```

## Differences from the SDK's VsixPublisher

- No Visual Studio telemetry.
- `help` and `version` print `Copyright (C) Christian Resma Helle` and this tool's own version.
- `-oidc` works. The original sends the GitHub token to the Marketplace token exchange as `Bearer`,
  which the Marketplace rejects. This port uses the same contract as `vsce`
  (`Authorization: FederatedToken`, `api-version=7.2-preview.1`, `X-Market-CI-Provider: github-actions`),
  and masks the resulting credential in GitHub Actions logs.
- Basic authentication over plain `http://` is refused (as in the original) except for loopback
  addresses, so you can test against a local mock server.
- The vsix-1.0 (`Vsix` 2010 schema) `Locale` LCID is mapped through a built-in table of common
  cultures instead of the Windows culture database.

## TLS and binary size

The release build is tuned for size (`opt-level = "z"`, LTO, `panic = "abort"`, stripped). It uses
a small blocking HTTP client, with no async runtime. On Windows and macOS it uses the operating
system's TLS stack and certificate store (SChannel / Security.framework). On Linux it uses rustls
with bundled Mozilla root certificates, so it doesn't need OpenSSL. `HTTPS_PROXY`/`NO_PROXY` are
honoured.

## Building

```shell
cargo build --release
cargo test
```
