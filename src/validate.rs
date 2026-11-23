//! Port of `Publisher.ValidatePublishManifest`.

use std::path::Path;

use crate::logger::Logger;
use crate::manifest::PublishManifest;
use crate::messages as msg;
use crate::vsix::is_valid_culture;

pub const MAX_DESCRIPTION: usize = 280;
pub const MAX_DISPLAY_NAME: usize = 80;
pub const MAX_INTERNAL_NAME: usize = 63;
pub const MAX_CATEGORIES: usize = 3;
pub const MAX_TAG: usize = 50;

/// .NET string length (UTF-16 code units).
fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

pub fn resolve(base: &Path, p: &str) -> std::path::PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

pub fn is_http_url(s: &str) -> bool {
    url::Url::parse(s)
        .map(|u| u.scheme() == "http" || u.scheme() == "https")
        .unwrap_or(false)
}

/// `^[a-zA-Z0-9][a-zA-Z0-9\-]*$`
fn is_valid_internal_name(s: &str) -> bool {
    let mut b = s.bytes();
    b.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && b.all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

/// Validates (and normalizes tags of) the manifest. `manifest_path` is used in messages.
pub fn validate(
    m: &mut PublishManifest,
    base_path: &Path,
    manifest_path: &str,
    log: &mut Logger,
) -> bool {
    if m.publisher.as_deref().unwrap_or_default().is_empty() {
        log.error(msg::MANIFEST_MISSING_PUBLISHER, &[&manifest_path]);
        return false;
    }
    let overview = m.overview.clone().unwrap_or_default();
    if overview.is_empty() || !resolve(base_path, &overview).is_file() {
        log.error(msg::LONG_DESCRIPTION_NOT_FOUND, &[&overview]);
        return false;
    }
    let n = m.categories.as_ref().map_or(0, Vec::len);
    if !(1..=MAX_CATEGORIES).contains(&n) {
        log.error(msg::TOO_MANY_OR_TOO_FEW_CATEGORIES, &[&MAX_CATEGORIES]);
        return false;
    }
    let id = m
        .identity
        .as_mut()
        .expect("identity resolved before validation");
    if let Some(tags) = id.tags.take() {
        let mut bad = false;
        let mut kept = Vec::new();
        for t in tags {
            if len16(&t) > MAX_TAG {
                log.error(msg::TAG_TOO_LARGE, &[&t, &MAX_TAG]);
                bad = true;
            } else if !t.is_empty() {
                kept.push(t);
            }
        }
        if bad {
            return false;
        }
        id.tags = Some(kept);
    }
    let desc = id.description.as_deref().unwrap_or_default();
    if desc.is_empty() || len16(desc) > MAX_DESCRIPTION {
        log.error(msg::DESCRIPTION_TOO_SHORT_OR_LONG, &[&MAX_DESCRIPTION]);
        return false;
    }
    let lang = id.language.as_deref().unwrap_or_default();
    if lang.is_empty() || !is_valid_culture(lang) {
        log.error(msg::LANGUAGE_MUST_BE_SET, &[]);
        return false;
    }
    let name = id.display_name.as_deref().unwrap_or_default();
    if name.is_empty() || len16(name) > MAX_DISPLAY_NAME {
        log.error(msg::DISPLAY_NAME_MUST_BE_SET, &[&MAX_DISPLAY_NAME]);
        return false;
    }
    if id.install_targets.as_ref().is_none_or(Vec::is_empty) {
        log.error(msg::REQUIRES_INSTALL_TARGET, &[]);
        return false;
    }
    if let Some(repo) = m.repo.as_deref().filter(|r| !r.is_empty())
        && !is_http_url(repo)
    {
        log.error(msg::SOURCE_REPO_INVALID, &[&repo]);
        return false;
    }
    let internal = id.internal_name.as_deref().unwrap_or_default();
    if !is_valid_internal_name(internal) || len16(internal) > MAX_INTERNAL_NAME {
        log.error(msg::INVALID_INTERNAL_NAME, &[&internal]);
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{Category, VisualStudioSku};
    use crate::logger::test_support::capture;
    use crate::manifest::{Identity, InstallTarget};
    use crate::version::VersionRange;

    fn valid(dir: &Path) -> PublishManifest {
        std::fs::write(dir.join("overview.md"), "# hi").unwrap();
        PublishManifest {
            publisher: Some("me".into()),
            overview: Some("overview.md".into()),
            categories: Some(vec![Category::Coding]),
            identity: Some(Identity {
                internal_name: Some("my-ext".into()),
                description: Some("desc".into()),
                display_name: Some("My Ext".into()),
                language: Some("en-US".into()),
                tags: Some(vec!["a".into(), "".into()]),
                install_targets: Some(vec![InstallTarget {
                    version_range: VersionRange::parse("[17.0,18.0)").unwrap(),
                    sku: VisualStudioSku::Community,
                }]),
                ..Default::default()
            }),
            repo: Some("https://github.com/me/x".into()),
            ..Default::default()
        }
    }

    fn idm(m: &mut PublishManifest) -> &mut Identity {
        m.identity.as_mut().unwrap()
    }

    fn code(dir: &Path, f: impl FnOnce(&mut PublishManifest)) -> i32 {
        let mut m = valid(dir);
        f(&mut m);
        let (mut log, _, _) = capture();
        if validate(&mut m, dir, "m.json", &mut log) {
            0
        } else {
            log.last_error_code_or(-1)
        }
    }

    #[test]
    fn valid_manifest_drops_empty_tags() {
        let d = tempfile::tempdir().unwrap();
        let mut m = valid(d.path());
        let (mut log, _, _) = capture();
        assert!(validate(&mut m, d.path(), "m.json", &mut log));
        assert_eq!(m.identity.unwrap().tags.unwrap(), vec!["a"]);
    }

    #[test]
    fn each_rule_has_its_code() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        assert_eq!(code(p, |m| m.publisher = None), 5);
        assert_eq!(code(p, |m| m.overview = Some("missing.md".into())), 9);
        assert_eq!(code(p, |m| m.categories = Some(vec![])), 21);
        assert_eq!(
            code(p, |m| m.categories = Some(vec![Category::Coding; 4])),
            21
        );
        assert_eq!(code(p, |m| idm(m).tags = Some(vec!["x".repeat(51)])), 23);
        assert_eq!(code(p, |m| idm(m).description = Some("x".repeat(281))), 24);
        assert_eq!(code(p, |m| idm(m).language = Some("klingon!".into())), 25);
        assert_eq!(code(p, |m| idm(m).display_name = None), 26);
        assert_eq!(code(p, |m| idm(m).install_targets = Some(vec![])), 27);
        assert_eq!(code(p, |m| m.repo = Some("ftp://x".into())), 28);
        assert_eq!(code(p, |m| idm(m).internal_name = Some("-bad".into())), 33);
        assert_eq!(code(p, |m| idm(m).internal_name = Some("a".repeat(64))), 33);
    }
}
