//! End-to-end tests of the binary against a mock marketplace.

use std::io::Write;
use std::path::Path;

use assert_cmd::Command;
use mockito::{Matcher, Server};
use predicates::prelude::*;

const V2_MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Id="MyExt.abc" Version="1.2.3" Language="en-US" Publisher="Me" />
    <DisplayName>My Extension</DisplayName>
    <Description xml:space="preserve">Does things.</Description>
  </Metadata>
  <Installation>
    <InstallationTarget Id="Microsoft.VisualStudio.Community" Version="[17.0,18.0)" />
  </Installation>
</PackageManifest>"#;

fn bin(home: &Path) -> Command {
    let mut c = Command::cargo_bin("vsixpublisher").unwrap();
    c.env("USERPROFILE", home).env("HOME", home);
    c
}

fn fixture(dir: &Path) {
    let f = std::fs::File::create(dir.join("My.vsix")).unwrap();
    let mut z = zip::ZipWriter::new(f);
    for (n, b) in [("[Content_Types].xml", "<Types/>"), ("extension.vsixmanifest", V2_MANIFEST)] {
        z.start_file(n, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b.as_bytes()).unwrap();
    }
    z.finish().unwrap();
    std::fs::write(dir.join("README.md"), "# Hi").unwrap();
    std::fs::write(
        dir.join("publish.json"),
        r#"{"publisher":"me","overview":"README.md","categories":["coding"],"identity":{"internalName":"my-ext"}}"#,
    )
    .unwrap();
}

#[test]
fn help_lists_commands() {
    let home = tempfile::tempdir().unwrap();
    bin(home.path())
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("publish                       Publishes an extension."));
}

#[test]
fn invalid_command_prints_usage_and_fails() {
    let home = tempfile::tempdir().unwrap();
    bin(home.path())
        .arg("nope")
        .assert()
        .failure()
        .stderr(predicate::str::contains("The 'nope' command is invalid."));
}

#[test]
fn login_publish_logout_round_trip() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    fixture(work.path());
    let mut s = Server::new();
    let url = format!("{}/", s.url());
    s.mock("GET", "/_apis/connectionData").match_query(Matcher::Any).with_body("{}").create();
    s.mock("GET", "/_apis/gallery/publishers/me/").with_body(r#"{"publisherName":"me"}"#).create();
    s.mock("GET", "/_apis/gallery/publishers/me/extensions/my-ext/")
        .with_status(404)
        .with_body(r#"{"message":"x","typeKey":"ExtensionDoesNotExistException"}"#)
        .create();
    let upload = s
        .mock("POST", "/_apis/gallery/publishers/me/extensions/")
        .match_header("authorization", "Basic OnNlY3JldA==")
        .with_body(r#"{"displayName":"My Extension"}"#)
        .create();

    bin(home.path())
        .args(["login", "-publisherName", "me", "-personalAccessToken", "secret", "-marketplaceUrl", &url])
        .assert()
        .success()
        .stdout("VSSDK: information VsixPub0041 : Publisher 'me' is now logged-in.\n");
    assert!(home.path().join(".vsce").is_file());

    bin(home.path())
        .current_dir(work.path())
        .args(["publish", "-payload", "My.vsix", "-publishManifest", "publish.json", "-marketplaceUrl", &url])
        .assert()
        .success()
        .stdout("VSSDK: information VsixPub0038 : Uploaded 'My Extension' to the marketplace.\n");
    upload.assert();

    bin(home.path())
        .args(["logout", "-publisherName", "me"])
        .assert()
        .success();
    assert!(!home.path().join(".vsce").exists());
}

#[test]
fn publish_without_login_fails_with_code_7() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    fixture(work.path());
    bin(home.path())
        .current_dir(work.path())
        .args(["publish", "-payload", "My.vsix", "-publishManifest", "publish.json"])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("Have you run the \"login\" command?"));
}
