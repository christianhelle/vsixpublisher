//! Port of `PublishCommand` + `Publisher`.

use std::path::{Path, PathBuf};

use reqwest::Url;

use crate::cli::{Args, Argument, ArgumentType::*, Command};
use crate::client::{parse_marketplace_url, ClientFactory, GetExtension, MarketplaceClient};
use crate::db::PublisherDb;
use crate::logger::Logger;
use crate::manifest::{AssetFile, ManifestError, PublishManifest};
use crate::marketplace_manifest::{self as mm, PayloadType};
use crate::messages as msg;
use crate::oidc::TokenProvider;
use crate::{package, validate, vsix};

pub struct PublishCommand {
    pub db_path: PathBuf,
    pub factory: Box<dyn ClientFactory>,
    pub tokens: Box<dyn TokenProvider>,
}

enum PayloadArg {
    Link,
    File(String),
    Invalid,
}

/// Classifies the `-payload` value like `Uri.TryCreate(..., Absolute)` would.
fn classify(payload: &str) -> PayloadArg {
    match Url::parse(payload) {
        Ok(u) if u.scheme() == "http" || u.scheme() == "https" => PayloadArg::Link,
        Ok(u) if u.scheme() == "file" => {
            PayloadArg::File(u.to_file_path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| payload.into()))
        }
        // Windows drive letters ("C:\...") parse as a one-letter scheme.
        Ok(u) if u.scheme().len() == 1 => PayloadArg::File(payload.into()),
        Ok(_) => PayloadArg::Invalid,
        Err(_) => PayloadArg::File(payload.into()),
    }
}

fn absolute(p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

/// An OPC package (what `Package.Open` accepts) is a zip with `[Content_Types].xml`.
fn is_vsix(path: &Path) -> bool {
    vsix::open(path).map(|z| z.index_for_name("[Content_Types].xml").is_some()).unwrap_or(false)
}

impl Command for PublishCommand {
    fn arguments(&self) -> Vec<Argument> {
        vec![
            Argument::new("payload", NonBoolean, msg::PUBLISHER_ARGUMENT_PAYLOAD).required(),
            Argument::new("publishManifest", NonBoolean, msg::PUBLISHER_ARGUMENT_PUBLISHER_MANIFEST).required(),
            Argument::new("personalAccessToken", NonBoolean, msg::PUBLISHER_ARGUMENT_PERSONAL_ACCESS_TOKEN),
            Argument::new("oidc", Boolean, msg::PUBLISHER_ARGUMENT_OIDC),
            Argument::new("outputFilePath", NonBoolean, msg::PUBLISHER_ARGUMENT_OUTPUT_FILE_PATH).hidden(),
            Argument::new("marketplaceUrl", NonBoolean, msg::ARGUMENT_MARKETPLACE_URL).hidden(),
            Argument::new("ignoreWarnings", NonBoolean, msg::PUBLISHER_ARGUMENT_IGNORE_WARNINGS),
        ]
    }

    fn help_text(&self) -> &'static str {
        msg::PUBLISHER_HELP_TEXT
    }

    fn validate(&self, args: &Args, log: &mut Logger) -> i32 {
        let payload = args.get("payload").unwrap_or_default();
        match classify(payload) {
            PayloadArg::Invalid => return log.error(msg::INVALID_PAYLOAD_ARGUMENT, &[&payload]),
            PayloadArg::File(p) if !Path::new(&p).is_file() => {
                return log.error(msg::CANNOT_FIND_PAYLOAD_PATH, &[&payload]);
            }
            _ => {}
        }
        let manifest = args.get("publishManifest").unwrap_or_default();
        if !Path::new(manifest).is_file() {
            return log.error(msg::CANNOT_FIND_PUBLISHER_MANIFEST_PATH, &[&manifest]);
        }
        0
    }

    fn execute(&self, args: &Args, log: &mut Logger) -> i32 {
        let payload_arg = args.get("payload").unwrap_or_default();
        let oidc = args.flag("oidc");
        let mut pat = args.get("personalAccessToken").map(str::to_string);
        if oidc && pat.as_deref().is_some_and(|p| !p.is_empty()) {
            return log.error(msg::CONFLICTING_AUTHENTICATION_ARGUMENTS, &[&"oidc", &"personalAccessToken"]);
        }
        let Some(url) = parse_marketplace_url(args.get("marketplaceUrl"), log) else {
            return log.last_error_code_or(-1);
        };

        let (payload, payload_type) = match classify(payload_arg) {
            PayloadArg::Link => (payload_arg.to_string(), PayloadType::Link),
            PayloadArg::File(p) => {
                let abs = absolute(&p);
                let t = if is_vsix(&abs) { PayloadType::Vsix } else { PayloadType::Other };
                (abs.to_string_lossy().into_owned(), t)
            }
            PayloadArg::Invalid => return log.error(msg::INVALID_PAYLOAD_ARGUMENT, &[&payload_arg]),
        };
        let manifest_path = absolute(args.get("publishManifest").unwrap_or_default());
        let manifest_display = manifest_path.to_string_lossy().into_owned();

        let mut manifest = match std::fs::read_to_string(&manifest_path)
            .map_err(|e| ManifestError::Invalid(e.to_string()))
            .and_then(|t| PublishManifest::from_json(&t))
        {
            Ok(m) => m,
            Err(ManifestError::Unsupported(e)) => return log.error(msg::MANIFEST_INVALID_DATA, &[&manifest_display, &e]),
            Err(ManifestError::Invalid(e)) => return log.error(msg::ERROR_READING_PUBLISHER_MANIFEST, &[&e]),
        };

        let publisher = manifest.publisher.clone().unwrap_or_default();
        if oidc {
            if publisher.is_empty() {
                return log.error(msg::MANIFEST_MISSING_PUBLISHER, &[&manifest_display]);
            }
            match self.tokens.session_token(&publisher, &url, log) {
                Some(t) => pat = Some(t),
                None => return log.last_error_code_or(-1),
            }
        } else if pat.as_deref().is_none_or(str::is_empty) {
            if publisher.is_empty() {
                // `TryGetPersonalAccessToken` throws on an empty name; surface it as a manifest error.
                return log.error(msg::MANIFEST_MISSING_PUBLISHER, &[&manifest_display]);
            }
            match PublisherDb::new(&self.db_path).personal_access_token(&publisher, log) {
                Some(p) => pat = Some(p),
                None => return log.last_error_code_or(-1),
            }
        }

        let client = match super::connect(self.factory.as_ref(), pat.as_deref().unwrap_or_default(), &url, log) {
            Ok(c) => c,
            Err(code) => return code,
        };
        let base = manifest_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let ctx = PublishContext {
            payload: &payload,
            payload_type,
            base: &base,
            manifest_display: &manifest_display,
            output: args.get("outputFilePath"),
            ignore_warnings: args.get("ignoreWarnings"),
        };
        publish(&ctx, &mut manifest, client.as_ref(), log)
    }
}

struct PublishContext<'a> {
    payload: &'a str,
    payload_type: PayloadType,
    base: &'a Path,
    manifest_display: &'a str,
    output: Option<&'a str>,
    ignore_warnings: Option<&'a str>,
}

/// Port of `Publisher.PublishCoreAsync`.
fn publish(ctx: &PublishContext, m: &mut PublishManifest, client: &dyn MarketplaceClient, log: &mut Logger) -> i32 {
    // Keeps the extracted icon alive until the upload finished.
    let mut _icon_file: Option<tempfile::TempPath> = None;

    if ctx.payload_type == PayloadType::Vsix {
        if m.identity.as_ref().and_then(|i| i.identifier.as_ref()).is_some() {
            return log.error(msg::MANIFEST_SHOULD_NOT_SET_IDENTITY, &[&ctx.payload]);
        }
        let internal_name = m.identity.as_ref().and_then(|i| i.internal_name.clone());
        let content = vsix::open(Path::new(ctx.payload)).ok().and_then(|mut z| vsix::manifest_content(&mut z));
        let Some(content) = content else {
            return log.error(msg::PAYLOAD_NOT_VALID_VSIX, &[&ctx.payload]);
        };
        match vsix::identity_from_manifest(&content, internal_name.as_deref()) {
            Ok(id) => m.identity = Some(id),
            Err(e) => return log.error(msg::ERROR_PARSING_VSIX_MANIFEST, &[&e]),
        }
        let icon = m.identity.as_ref().and_then(|i| i.icon.clone()).filter(|s| !s.is_empty());
        if let Some(icon) = icon {
            let tmp = match tempfile::Builder::new().prefix("tmp").suffix(".tmp").tempfile() {
                Ok(t) => t.into_temp_path(),
                Err(e) => return log.error(msg::GENERIC_ERROR_PUBLISHING, &[&ctx.payload, &e]),
            };
            if !matches!(vsix::extract(Path::new(ctx.payload), &icon, &tmp), Ok(true)) {
                return log.error(msg::CANNOT_FIND_FILE_IN_VSIX, &[&icon, &ctx.payload]);
            }
            m.identity.as_mut().unwrap().icon = Some(tmp.to_string_lossy().into_owned());
            _icon_file = Some(tmp);
        }
    } else if m.identity.is_none() {
        return log.error(msg::PAYLOAD_REQUIRES_IDENTITY, &[&ctx.payload]);
    }

    if !validate::validate(m, ctx.base, ctx.manifest_display, log) {
        return log.last_error_code_or(-1);
    }

    m.asset_files.push(AssetFile {
        path_on_disk: m.overview.clone().unwrap_or_default(),
        target_path: mm::OVERVIEW_PATH.into(),
        asset_type: Some(mm::OVERVIEW_TYPE.into()),
    });
    if let Some(icon) = m.identity.as_ref().and_then(|i| i.icon.clone()).filter(|s| !s.is_empty()) {
        m.asset_files.push(AssetFile {
            target_path: mm::ensure_leading_slash(mm::file_name(&icon)),
            path_on_disk: icon,
            asset_type: Some(mm::ICON_TYPE.into()),
        });
    }
    if let Some(w) = ctx.ignore_warnings.filter(|w| !w.is_empty()) {
        m.ignore_warnings = Some(w.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect());
    }

    let xml = mm::generate(ctx.payload, ctx.payload_type, m);
    let payload_path = (ctx.payload_type != PayloadType::Link).then(|| Path::new(ctx.payload));
    let Some(pkg) = package::build(m, &xml, payload_path, ctx.base, log) else {
        return log.last_error_code_or(-1);
    };

    if let Some(out) = ctx.output.filter(|o| !o.is_empty()) {
        if let Err(e) = std::fs::write(out, &pkg) {
            return log.error(msg::GENERIC_ERROR_PUBLISHING, &[&ctx.payload, &e]);
        }
    }

    let publisher = m.publisher.clone().unwrap_or_default();
    let internal = m.identity.as_ref().and_then(|i| i.internal_name.clone()).unwrap_or_default();
    let uploaded = match client.get_extension(&publisher, &internal, log) {
        GetExtension::Success(_) => client.update_extension(&pkg, &publisher, &internal, log),
        GetExtension::NotFound => client.create_extension(&pkg, &publisher, log),
        GetExtension::Error => return log.last_error_code_or(-1),
    };
    let response = match uploaded {
        Ok(r) => r,
        Err(e) => return log.error(msg::GENERIC_ERROR_PUBLISHING, &[&ctx.payload, &e]),
    };
    if let Some(e) = log.last_error() {
        return e.error_code();
    }
    match response {
        Some(r) => {
            log.info(msg::INFO_EXTENSION_UPLOADED, &[&r.display_name.unwrap_or_default()]);
            0
        }
        None => log.error(
            msg::GENERIC_ERROR_PUBLISHING,
            &[&ctx.payload, &"No errors have been logged but the response from the marketplace could not be parsed."],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ExtensionResponse;
    use crate::commands::fake::{args, Fake};
    use crate::logger::test_support::capture;
    use crate::vsix::test_support::{write_vsix, V2_MANIFEST};
    use std::io::Read;

    struct NoTokens;
    impl TokenProvider for NoTokens {
        fn session_token(&self, _: &str, _: &Url, log: &mut Logger) -> Option<String> {
            log.error(msg::OIDC_PROVIDER_UNAVAILABLE, &[&"a", &"b"]);
            None
        }
    }

    struct Tokens;
    impl TokenProvider for Tokens {
        fn session_token(&self, p: &str, _: &Url, _: &mut Logger) -> Option<String> {
            Some(format!("session-for-{p}"))
        }
    }

    struct Env {
        dir: tempfile::TempDir,
        fake: Fake,
    }

    impl Env {
        fn new(manifest_json: &str) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let p = dir.path();
            write_vsix(
                &p.join("My.vsix"),
                &[
                    ("[Content_Types].xml", b"<Types/>"),
                    ("extension.vsixmanifest", V2_MANIFEST.as_bytes()),
                    ("Resources/icon.png", b"PNG"),
                ],
            );
            std::fs::write(p.join("README.md"), "# Hello").unwrap();
            std::fs::write(p.join("setup.exe"), "MZ").unwrap();
            std::fs::write(p.join("publish.json"), manifest_json).unwrap();
            let fake = Fake::default();
            fake.0.borrow_mut().upload = Some(ExtensionResponse { display_name: Some("My Extension".into()), ..Default::default() });
            Env { dir, fake }
        }

        fn path(&self, f: &str) -> String {
            self.dir.path().join(f).to_string_lossy().into_owned()
        }

        fn cmd(&self, tokens: Box<dyn TokenProvider>) -> PublishCommand {
            PublishCommand { db_path: self.dir.path().join(".vsce"), factory: Box::new(self.fake.clone()), tokens }
        }

        fn run(&self, extra: &[(&'static str, Option<&str>)]) -> (i32, String, String) {
            self.run_with(Box::new(NoTokens), extra)
        }

        fn run_with(&self, tokens: Box<dyn TokenProvider>, extra: &[(&'static str, Option<&str>)]) -> (i32, String, String) {
            let payload = self.path("My.vsix");
            let manifest = self.path("publish.json");
            let mut pairs: Vec<(&'static str, Option<&str>)> =
                vec![("payload", Some(&payload)), ("publishManifest", Some(&manifest))];
            for (k, v) in extra {
                pairs.retain(|(pk, _)| pk != k);
                pairs.push((k, *v));
            }
            let a = args(&pairs);
            let (mut log, out, err) = capture();
            let c = self.cmd(tokens);
            let mut code = c.validate(&a, &mut log);
            if code == 0 {
                code = c.execute(&a, &mut log);
            }
            (code, out.text(), err.text())
        }

        fn uploaded_entry(&self, name: &str) -> Vec<u8> {
            let bytes = self.fake.0.borrow().uploaded.clone();
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
            let mut f = z.by_name(name).unwrap();
            let mut b = Vec::new();
            f.read_to_end(&mut b).unwrap();
            b
        }
    }

    const VSIX_JSON: &str = r#"{"publisher":"me","overview":"README.md","categories":["coding"],"identity":{"internalName":"my-ext"}}"#;

    #[test]
    fn publishes_new_vsix_extension() {
        let env = Env::new(VSIX_JSON);
        let (code, out, err) = env.run(&[("personalAccessToken", Some("pat"))]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "VSSDK: information VsixPub0038 : Uploaded 'My Extension' to the marketplace.\n");
        let s = env.fake.0.borrow();
        assert_eq!(s.pat.as_deref(), Some("pat"));
        assert_eq!(s.calls, vec!["get_extension me my-ext", "create_extension me"]);
        drop(s);
        let xml = String::from_utf8(env.uploaded_entry("extension.marketplacemanifest")).unwrap();
        assert!(xml.contains(r#"<Identity Language="en-US" Id="my-ext" Version="1.2.3" Publisher="me" />"#));
        assert!(xml.contains("<VsixId>MyExt.abc</VsixId>"));
        assert!(xml.contains(r#"Path="/My.vsix""#));
        assert!(!xml.contains("<Installation>"));
        assert_eq!(env.uploaded_entry("overview.md"), b"# Hello");
        assert_eq!(env.uploaded_entry("My.vsix").len() > 0, true);
        let icon_line = xml.lines().find(|l| l.contains("<Icon>")).unwrap();
        let icon = icon_line.trim().trim_start_matches("<Icon>/").trim_end_matches("</Icon>");
        assert_eq!(env.uploaded_entry(icon), b"PNG");
    }

    #[test]
    fn updates_existing_extension_and_writes_output_file() {
        let env = Env::new(VSIX_JSON);
        env.fake.0.borrow_mut().get_extension = Some(GetExtension::Success(ExtensionResponse::default()));
        let out_file = env.path("out.zip");
        let (code, _, _) = env.run(&[("personalAccessToken", Some("pat")), ("outputFilePath", Some(&out_file))]);
        assert_eq!(code, 0);
        assert_eq!(env.fake.0.borrow().calls[1], "update_extension me my-ext");
        assert_eq!(std::fs::read(&out_file).unwrap(), env.fake.0.borrow().uploaded);
    }

    #[test]
    fn uses_stored_pat_or_fails() {
        let env = Env::new(VSIX_JSON);
        let (code, _, err) = env.run(&[]);
        assert_eq!(code, 7, "{err}");
        let (mut log, _, _) = capture();
        PublisherDb::new(env.dir.path().join(".vsce")).add("me", "stored", false, &mut log);
        assert_eq!(env.run(&[]).0, 0);
        assert_eq!(env.fake.0.borrow().pat.as_deref(), Some("stored"));
    }

    #[test]
    fn oidc_flow() {
        let env = Env::new(VSIX_JSON);
        assert_eq!(env.run(&[("oidc", None), ("personalAccessToken", Some("p"))]).0, 44);
        assert_eq!(env.run(&[("oidc", None)]).0, 45);
        assert_eq!(env.run_with(Box::new(Tokens), &[("oidc", None)]).0, 0);
        assert_eq!(env.fake.0.borrow().pat.as_deref(), Some("session-for-me"));
    }

    #[test]
    fn validation_errors() {
        let env = Env::new(VSIX_JSON);
        assert_eq!(env.run(&[("payload", Some("ftp://x/y.vsix"))]).0, 22);
        assert_eq!(env.run(&[("payload", Some("missing.vsix"))]).0, 1);
        assert_eq!(env.run(&[("publishManifest", Some("missing.json"))]).0, 2);
        assert_eq!(env.run(&[("personalAccessToken", Some("p")), ("marketplaceUrl", Some("::"))]).0, 34);
    }

    #[test]
    fn manifest_errors() {
        let env = Env::new("{");
        assert_eq!(env.run(&[("personalAccessToken", Some("p"))]).0, 3);
        let env = Env::new(r#"{"publisher":"me","overview":"README.md","categories":["bogus"],"identity":{"internalName":"x"}}"#);
        assert_eq!(env.run(&[("personalAccessToken", Some("p"))]).0, 6);
        let env = Env::new(r#"{"publisher":"me","overview":"README.md","categories":["coding"],"identity":{"internalName":"x","vsixId":"y"}}"#);
        assert_eq!(env.run(&[("personalAccessToken", Some("p"))]).0, 17);
        let env = Env::new(r#"{"publisher":"me","overview":"NOPE.md","categories":["coding"],"identity":{"internalName":"x"}}"#);
        assert_eq!(env.run(&[("personalAccessToken", Some("p"))]).0, 9);
    }

    #[test]
    fn exe_payload_requires_identity_and_emits_installation() {
        let env = Env::new(VSIX_JSON);
        let exe = env.path("setup.exe");
        assert_eq!(env.run(&[("personalAccessToken", Some("p")), ("payload", Some(&exe))]).0, 24);
        // VSIX_JSON identity lacks description etc. -> validation error.
        let env = Env::new(
            r#"{"publisher":"me","overview":"README.md","categories":["coding"],
               "identity":{"internalName":"my-tool","displayName":"Tool","description":"A tool","language":"en-US",
               "version":"1.0","installTargets":[{"sku":"Community","version":"[17.0,18.0)"}]}}"#,
        );
        let exe = env.path("setup.exe");
        let (code, _, err) = env.run(&[("personalAccessToken", Some("p")), ("payload", Some(&exe)), ("ignoreWarnings", Some("W1, W2"))]);
        assert_eq!(code, 0, "{err}");
        let xml = String::from_utf8(env.uploaded_entry("extension.marketplacemanifest")).unwrap();
        assert!(xml.contains(r#"<InstallationTarget Id="Microsoft.VisualStudio.Community" Version="[17.0,18.0)" />"#));
        assert!(xml.contains("<IgnoreWarnings>W1,W2</IgnoreWarnings>"));
        assert_eq!(env.uploaded_entry("setup.exe"), b"MZ");
    }

    #[test]
    fn link_payload() {
        let env = Env::new(
            r#"{"publisher":"me","overview":"README.md","categories":["coding"],
               "identity":{"internalName":"my-link","displayName":"Link","description":"d","language":"en-US",
               "installTargets":[{"sku":"Pro","version":"[17.0,18.0)"}]}}"#,
        );
        let (code, _, err) = env.run(&[("personalAccessToken", Some("p")), ("payload", Some("https://example.com/get"))]);
        assert_eq!(code, 0, "{err}");
        let xml = String::from_utf8(env.uploaded_entry("extension.marketplacemanifest")).unwrap();
        assert!(xml.contains(r#"Value="https://example.com/get""#));
    }

    #[test]
    fn unparseable_marketplace_response_is_an_error() {
        let env = Env::new(VSIX_JSON);
        env.fake.0.borrow_mut().upload = None;
        assert_eq!(env.run(&[("personalAccessToken", Some("p"))]).0, 4);
    }
}
