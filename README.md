# vsixpublisher

A cross-platform Rust port of `VsixPublisher.exe`, the tool in the Visual Studio SDK that publishes
extensions to the [Visual Studio Marketplace](https://marketplace.visualstudio.com/).

The original is a .NET Framework 4.7.2 app that ships with ~40 assemblies and only runs on Windows.
This port is a single native binary for Windows, Linux and macOS. It's meant as a drop-in replacement:
same commands, same `-argument value` syntax, same messages and `VsixPubNNNN` error codes, same exit
codes, and the same `~/.vsce` publisher store.

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

## Differences from the original

- No Visual Studio telemetry.
- `-oidc` works. The original sends the GitHub token to the Marketplace token exchange as `Bearer`,
  which the Marketplace rejects. This port uses the same contract as `vsce`
  (`Authorization: FederatedToken`, `api-version=7.2-preview.1`, `X-Market-CI-Provider: github-actions`),
  and masks the resulting credential in GitHub Actions logs.
- Basic authentication over plain `http://` is refused (as in the original) except for loopback
  addresses, so you can test against a local mock server.
- The vsix-1.0 (`Vsix` 2010 schema) `Locale` LCID is mapped through a built-in table of common
  cultures instead of the Windows culture database.

## Building

```shell
cargo build --release
cargo test
```
