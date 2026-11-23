//! Publish manifest data contracts, deserialized with Newtonsoft-like semantics
//! (case-insensitive property names, `IsRequired` members).

use serde_json::{Map, Value};

use crate::enums::{Category, PriceCategory, VisualStudioSku};
use crate::version::{Version, VersionRange};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTarget {
    pub version_range: VersionRange,
    pub sku: VisualStudioSku,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identity {
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub icon: Option<String>,
    pub identifier: Option<String>,
    pub install_targets: Option<Vec<InstallTarget>>,
    pub internal_name: Option<String>,
    pub language: Option<String>,
    pub tags: Option<Vec<String>>,
    pub version: Option<Version>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetFile {
    pub path_on_disk: String,
    pub target_path: String,
    /// Not serialized; set for overview/icon assets.
    pub asset_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishManifest {
    pub identity: Option<Identity>,
    pub asset_files: Vec<AssetFile>,
    pub categories: Option<Vec<Category>>,
    pub ignore_warnings: Option<Vec<String>>,
    pub overview: Option<String>,
    pub price_category: PriceCategory,
    pub is_private: bool,
    pub publisher: Option<String>,
    pub qna: bool,
    pub repo: Option<String>,
}

impl Default for PublishManifest {
    fn default() -> Self {
        Self {
            identity: None,
            asset_files: Vec::new(),
            categories: None,
            ignore_warnings: None,
            overview: None,
            price_category: PriceCategory::Free,
            is_private: false,
            publisher: None,
            qna: true,
            repo: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Maps to `NotSupportedException` (VsixPub0006).
    Unsupported(String),
    /// Maps to JSON reader/serialization errors (VsixPub0003).
    Invalid(String),
}

type R<T> = Result<T, ManifestError>;

fn invalid<T>(m: impl Into<String>) -> R<T> {
    Err(ManifestError::Invalid(m.into()))
}

/// Case-insensitive property lookup preferring an exact match.
fn prop<'a>(obj: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    obj.get(name).or_else(|| {
        obj.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    })
}

fn required<'a>(obj: &'a Map<String, Value>, name: &str) -> R<&'a Value> {
    prop(obj, name).map_or_else(
        || invalid(format!("Required property '{name}' not found in JSON.")),
        Ok,
    )
}

fn as_object<'a>(v: &'a Value, what: &str) -> R<&'a Map<String, Value>> {
    v.as_object()
        .map_or_else(|| invalid(format!("Expected an object for '{what}'.")), Ok)
}

fn as_opt_string(v: Option<&Value>, what: &str) -> R<Option<String>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(Value::Bool(b)) => Ok(Some(if *b { "True" } else { "False" }.into())),
        Some(_) => invalid(format!("Unexpected value for '{what}'.")),
    }
}

fn as_string_value(v: &Value, what: &str) -> R<String> {
    Ok(as_opt_string(Some(v), what)?.unwrap_or_default())
}

fn as_opt_array<'a>(v: Option<&'a Value>, what: &str) -> R<Option<&'a Vec<Value>>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) => Ok(Some(a)),
        Some(_) => invalid(format!("Expected an array for '{what}'.")),
    }
}

fn as_bool(v: Option<&Value>, default: bool, what: &str) -> R<bool> {
    match v {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::String(s)) if s.eq_ignore_ascii_case("true") => Ok(true),
        Some(Value::String(s)) if s.eq_ignore_ascii_case("false") => Ok(false),
        Some(_) => invalid(format!(
            "Could not convert value for '{what}' to a boolean."
        )),
    }
}

fn string_list(v: Option<&Value>, what: &str) -> R<Option<Vec<String>>> {
    as_opt_array(v, what)?
        .map(|a| a.iter().map(|x| as_string_value(x, what)).collect())
        .transpose()
}

fn parse_install_target(v: &Value) -> R<InstallTarget> {
    let o = as_object(v, "installTargets")?;
    let ver = required(o, "version")?;
    let version_range = match ver {
        Value::String(s) => VersionRange::parse(s).map_or_else(
            || {
                invalid(format!(
                    "Error parsing version range - \"{s}\" is not a valid version range."
                ))
            },
            Ok,
        )?,
        _ => {
            return invalid(
                "Error parsing version range. Expected a version range but got a different type.",
            );
        }
    };
    let sku = as_string_value(required(o, "sku")?, "sku")?;
    let sku = VisualStudioSku::parse(&sku).map_err(ManifestError::Unsupported)?;
    Ok(InstallTarget { version_range, sku })
}

fn parse_identity(v: &Value) -> R<Option<Identity>> {
    if v.is_null() {
        return Ok(None);
    }
    let o = as_object(v, "identity")?;
    let internal_name = as_opt_string(Some(required(o, "internalName")?), "internalName")?;
    let version = match as_opt_string(prop(o, "version"), "version")? {
        None => None,
        Some(s) => Some(Version::parse(&s).map_or_else(
            || {
                invalid(format!(
                    "Error converting value \"{s}\" to type 'System.Version'."
                ))
            },
            Ok,
        )?),
    };
    let install_targets = as_opt_array(prop(o, "installTargets"), "installTargets")?
        .map(|a| a.iter().map(parse_install_target).collect::<R<Vec<_>>>())
        .transpose()?;
    Ok(Some(Identity {
        description: as_opt_string(prop(o, "description"), "description")?,
        display_name: as_opt_string(prop(o, "displayName"), "displayName")?,
        icon: as_opt_string(prop(o, "icon"), "icon")?,
        identifier: as_opt_string(prop(o, "vsixId"), "vsixId")?,
        install_targets,
        internal_name,
        language: as_opt_string(prop(o, "language"), "language")?,
        tags: string_list(prop(o, "tags"), "tags")?,
        version,
    }))
}

fn parse_asset_file(v: &Value) -> R<AssetFile> {
    let o = as_object(v, "assetFiles")?;
    Ok(AssetFile {
        path_on_disk: as_string_value(required(o, "pathOnDisk")?, "pathOnDisk")?,
        target_path: as_string_value(required(o, "targetPath")?, "targetPath")?,
        asset_type: None,
    })
}

impl PublishManifest {
    pub fn from_json(text: &str) -> R<Self> {
        let root: Value =
            serde_json::from_str(text).map_err(|e| ManifestError::Invalid(e.to_string()))?;
        let o = as_object(&root, "manifest")?;
        let identity = parse_identity(required(o, "identity")?)?;
        let categories = match as_opt_array(Some(required(o, "categories")?), "categories")? {
            None => None,
            Some(a) => Some(
                a.iter()
                    .map(|c| {
                        let s = as_string_value(c, "categories")?;
                        Category::parse(&s).map_err(ManifestError::Unsupported)
                    })
                    .collect::<R<Vec<_>>>()?,
            ),
        };
        let asset_files = as_opt_array(prop(o, "assetFiles"), "assetFiles")?
            .map(|a| a.iter().map(parse_asset_file).collect::<R<Vec<_>>>())
            .transpose()?
            .unwrap_or_default();
        let price_category = match prop(o, "priceCategory") {
            None | Some(Value::Null) => PriceCategory::Free,
            Some(v) => PriceCategory::parse(&as_string_value(v, "priceCategory")?)
                .map_err(ManifestError::Invalid)?,
        };
        Ok(Self {
            identity,
            asset_files,
            categories,
            ignore_warnings: string_list(prop(o, "ignoreWarnings"), "ignoreWarnings")?,
            overview: as_opt_string(Some(required(o, "overview")?), "overview")?,
            price_category,
            is_private: as_bool(prop(o, "private"), false, "private")?,
            publisher: as_opt_string(Some(required(o, "publisher")?), "publisher")?,
            qna: as_bool(prop(o, "qna"), true, "qna")?,
            repo: as_opt_string(prop(o, "repo"), "repo")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VSIX_MANIFEST: &str = r#"{
        "$schema": "http://json.schemastore.org/vsix-publish",
        "categories": [ "build", "coding" ],
        "identity": { "internalName": "my-ext" },
        "overview": "README.md",
        "priceCategory": "free",
        "publisher": "me",
        "private": false,
        "qna": true,
        "repo": "https://github.com/me/my-ext",
        "assetFiles": [ { "pathOnDisk": "img/a.png", "targetPath": "img/a.png" } ]
    }"#;

    #[test]
    fn parses_vsix_style_manifest() {
        let m = PublishManifest::from_json(VSIX_MANIFEST).unwrap();
        assert_eq!(m.publisher.as_deref(), Some("me"));
        assert_eq!(m.categories, Some(vec![Category::Build, Category::Coding]));
        assert_eq!(m.identity.unwrap().internal_name.as_deref(), Some("my-ext"));
        assert_eq!(m.asset_files[0].target_path, "img/a.png");
        assert_eq!(m.repo.as_deref(), Some("https://github.com/me/my-ext"));
    }

    #[test]
    fn keys_are_case_insensitive_and_defaults_apply() {
        let m = PublishManifest::from_json(
            r#"{"Publisher":"p","OVERVIEW":"o.md","Categories":["other"],"Identity":{"InternalName":"x"}}"#,
        )
        .unwrap();
        assert_eq!(m.publisher.as_deref(), Some("p"));
        assert!(m.qna);
        assert!(!m.is_private);
        assert_eq!(m.price_category, PriceCategory::Free);
    }

    #[test]
    fn parses_full_identity() {
        let m = PublishManifest::from_json(
            r#"{"publisher":"p","overview":"o.md","categories":["other"],"priceCategory":"Trial",
                "identity":{"internalName":"x","vsixId":"id","version":"1.2.3","displayName":"X",
                "description":"d","language":"en-US","tags":["a","b"],"icon":"i.png",
                "installTargets":[{"sku":"Community","version":"[17.0,18.0)"}]}}"#,
        )
        .unwrap();
        let id = m.identity.unwrap();
        assert_eq!(id.version, Version::parse("1.2.3"));
        assert_eq!(id.tags.unwrap(), vec!["a", "b"]);
        let t = &id.install_targets.unwrap()[0];
        assert_eq!(t.sku, VisualStudioSku::Community);
        assert_eq!(t.version_range.to_string(), "[17.0,18.0)");
        assert_eq!(m.price_category, PriceCategory::Trial);
    }

    #[test]
    fn missing_required_is_invalid() {
        let e = PublishManifest::from_json(r#"{"publisher":"p","overview":"o","categories":[]}"#)
            .unwrap_err();
        assert_eq!(
            e,
            ManifestError::Invalid("Required property 'identity' not found in JSON.".into())
        );
    }

    #[test]
    fn bad_category_or_sku_is_unsupported() {
        let e = PublishManifest::from_json(
            r#"{"publisher":"p","overview":"o","categories":["nope"],"identity":{"internalName":"x"}}"#,
        )
        .unwrap_err();
        assert!(matches!(e, ManifestError::Unsupported(_)));
        let e = PublishManifest::from_json(
            r#"{"publisher":"p","overview":"o","categories":["other"],"identity":{"internalName":"x","installTargets":[{"sku":"foo","version":"17.0"}]}}"#,
        )
        .unwrap_err();
        assert_eq!(
            e,
            ManifestError::Unsupported("Could not find matching sku for foo.".into())
        );
    }

    #[test]
    fn malformed_json_is_invalid() {
        assert!(matches!(
            PublishManifest::from_json("{"),
            Err(ManifestError::Invalid(_))
        ));
    }
}
