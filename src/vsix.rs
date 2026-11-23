//! Reading VSIX payloads: identity from `extension.vsixmanifest`, file extraction.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::enums::VisualStudioSku;
use crate::manifest::{Identity, InstallTarget};
use crate::version::{Version, VersionRange};
use crate::xml::{self, Element};

pub const NS_2011: &str = "http://schemas.microsoft.com/developer/vsx-schema/2011";
pub const NS_2010: &str = "http://schemas.microsoft.com/developer/vsx-schema/2010";
const MANIFEST_PART: &str = "/extension.vsixmanifest";

pub type Archive = zip::ZipArchive<File>;

pub fn open(path: &Path) -> Result<Archive, String> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(f).map_err(|e| e.to_string())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Finds a zip entry for an OPC part name (case-insensitive, percent-decoded).
fn find_part(zip: &Archive, part: &str) -> Option<usize> {
    let want = percent_decode(part.trim_start_matches('/'));
    (0..zip.len()).find(|&i| {
        matches!(zip.name_for_index(i), Some(Ok(n)) if percent_decode(&n).eq_ignore_ascii_case(&want))
    })
}

pub fn read_part(zip: &mut Archive, part: &str) -> Option<Vec<u8>> {
    let i = find_part(zip, part)?;
    let mut f = zip.by_index(i).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Returns the manifest text, or `None` when the payload has no vsixmanifest.
pub fn manifest_content(zip: &mut Archive) -> Option<String> {
    read_part(zip, MANIFEST_PART).map(|b| decode_text(&b))
}

/// `StreamReader` default decoding: honours BOMs, else UTF-8.
fn decode_text(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    String::from_utf8_lossy(b).into_owned()
}

/// Extracts `part` into `out`. Returns `Ok(false)` when the part does not exist.
pub fn extract(vsix: &Path, part: &str, out: &Path) -> Result<bool, String> {
    let mut zip = open(vsix)?;
    let mut f = File::create(out).map_err(|e| e.to_string())?;
    match read_part(&mut zip, part) {
        Some(b) => {
            f.write_all(&b).map_err(|e| e.to_string())?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn ensure_leading_slash(p: &str) -> String {
    if p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

fn normalize_icon(v: &str) -> String {
    ensure_leading_slash(&v.replace('\\', "/"))
}

fn child<'a>(e: &'a Element, ns: &'a str, name: &'a str) -> Result<&'a Element, String> {
    e.element(ns, name)
        .ok_or_else(|| format!("The '{name}' element is missing from the vsixmanifest."))
}

fn attr<'a>(e: &'a Element, name: &str) -> Result<&'a str, String> {
    e.attr(name)
        .ok_or_else(|| format!("The '{name}' attribute is missing on '{}'.", e.name))
}

fn version(v: &str) -> Result<Version, String> {
    Version::parse(v).ok_or_else(|| format!("Version string '{v}' is not valid."))
}

fn range(v: &str) -> Result<VersionRange, String> {
    VersionRange::parse(v).ok_or_else(|| format!("Version range '{v}' is not valid."))
}

fn tags(e: &Element, ns: &str) -> Option<Vec<String>> {
    e.element(ns, "Tags").map(|t| {
        t.value()
            .split(';')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn icon(e: &Element, ns: &str) -> Option<String> {
    e.element(ns, "Icon")
        .map(Element::value)
        .filter(|v| !v.is_empty())
        .map(|v| normalize_icon(&v))
}

/// Port of `Identity.Create`.
pub fn identity_from_manifest(
    content: &str,
    internal_name: Option<&str>,
) -> Result<Identity, String> {
    let internal_name = internal_name
        .filter(|n| !n.is_empty())
        .ok_or("The internal name of the extension is required.")?;
    let root = xml::parse(content)?;
    let mut id = if root.is(NS_2011, "PackageManifest") {
        from_package_manifest(&root)?
    } else if root.is(NS_2010, "Vsix") {
        from_vsix_manifest(&root)?
    } else {
        return Err("The vsixmanifest for the given payload is invalid.".into());
    };
    id.internal_name = Some(internal_name.to_string());
    Ok(id)
}

fn from_package_manifest(root: &Element) -> Result<Identity, String> {
    let md = child(root, NS_2011, "Metadata")?;
    let ident = child(md, NS_2011, "Identity")?;
    let mut targets = Vec::new();
    if let Some(inst) = root.element(NS_2011, "Installation") {
        for t in inst.elements(NS_2011, "InstallationTarget") {
            targets.push(InstallTarget {
                version_range: range(attr(t, "Version")?)?,
                sku: VisualStudioSku::parse(attr(t, "Id")?)?,
            });
        }
    }
    Ok(Identity {
        identifier: Some(attr(ident, "Id")?.to_string()),
        version: Some(version(attr(ident, "Version")?)?),
        language: Some(attr(ident, "Language")?.to_string()),
        description: Some(child(md, NS_2011, "Description")?.value()),
        display_name: Some(child(md, NS_2011, "DisplayName")?.value()),
        icon: icon(md, NS_2011),
        tags: tags(md, NS_2011),
        install_targets: Some(targets),
        internal_name: None,
    })
}

fn from_vsix_manifest(root: &Element) -> Result<Identity, String> {
    let ident = child(root, NS_2010, "Identifier")?;
    let locale = child(ident, NS_2010, "Locale")?.value();
    let lcid: u32 = locale
        .trim()
        .parse()
        .map_err(|_| format!("Locale '{locale}' is not valid."))?;
    let language = culture_name_for_lcid(lcid)
        .ok_or_else(|| format!("Culture is not supported. Parameter name: culture {lcid} is an invalid culture identifier."))?;
    let mut targets = Vec::new();
    if let Some(sp) = ident.element(NS_2010, "SupportedProducts") {
        for vs in sp.elements(NS_2010, "VisualStudio") {
            for ed in vs.elements(NS_2010, "Edition") {
                targets.push(InstallTarget {
                    version_range: range(attr(vs, "Version")?)?,
                    sku: VisualStudioSku::parse(&ed.value())?,
                });
            }
        }
    }
    Ok(Identity {
        identifier: Some(attr(ident, "Id")?.to_string()),
        version: Some(version(&child(ident, NS_2010, "Version")?.value())?),
        language: Some(language.to_string()),
        description: Some(child(ident, NS_2010, "Description")?.value()),
        display_name: Some(child(ident, NS_2010, "Name")?.value()),
        icon: icon(ident, NS_2010),
        tags: tags(ident, NS_2010),
        install_targets: Some(targets),
        internal_name: None,
    })
}

const LCIDS: &[(u32, &str)] = &[
    (127, ""),
    (1025, "ar-SA"),
    (1026, "bg-BG"),
    (1027, "ca-ES"),
    (1028, "zh-TW"),
    (1029, "cs-CZ"),
    (1030, "da-DK"),
    (1031, "de-DE"),
    (1032, "el-GR"),
    (1033, "en-US"),
    (1035, "fi-FI"),
    (1036, "fr-FR"),
    (1037, "he-IL"),
    (1038, "hu-HU"),
    (1039, "is-IS"),
    (1040, "it-IT"),
    (1041, "ja-JP"),
    (1042, "ko-KR"),
    (1043, "nl-NL"),
    (1044, "nb-NO"),
    (1045, "pl-PL"),
    (1046, "pt-BR"),
    (1048, "ro-RO"),
    (1049, "ru-RU"),
    (1050, "hr-HR"),
    (1051, "sk-SK"),
    (1053, "sv-SE"),
    (1054, "th-TH"),
    (1055, "tr-TR"),
    (1057, "id-ID"),
    (1058, "uk-UA"),
    (1060, "sl-SI"),
    (1061, "et-EE"),
    (1062, "lv-LV"),
    (1063, "lt-LT"),
    (1066, "vi-VN"),
    (1081, "hi-IN"),
    (2052, "zh-CN"),
    (2057, "en-GB"),
    (2058, "es-MX"),
    (2070, "pt-PT"),
    (3076, "zh-HK"),
    (3081, "en-AU"),
    (3082, "es-ES"),
    (3084, "fr-CA"),
    (4105, "en-CA"),
    (5129, "en-NZ"),
    (6153, "en-IE"),
    (2055, "de-CH"),
    (3079, "de-AT"),
    (2067, "nl-BE"),
    (2060, "fr-BE"),
    (4108, "fr-CH"),
    (9, "en"),
    (7, "de"),
    (12, "fr"),
    (10, "es"),
    (16, "it"),
    (17, "ja"),
    (18, "ko"),
    (22, "pt"),
    (25, "ru"),
    (4, "zh-Hans"),
    (31748, "zh-Hant"),
];

pub fn culture_name_for_lcid(lcid: u32) -> Option<&'static str> {
    LCIDS.iter().find(|(l, _)| *l == lcid).map(|(_, n)| *n)
}

/// Approximates `new CultureInfo(name)`: LCID integers or BCP-47 shaped names.
pub fn is_valid_culture(s: &str) -> bool {
    if let Ok(n) = s.trim().parse::<u32>() {
        return culture_name_for_lcid(n).is_some();
    }
    // language[-Script][-REGION|-123][-variant...]
    let alpha = |p: &str, n: std::ops::RangeInclusive<usize>| {
        n.contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphabetic())
    };
    let mut parts = s.split('-').peekable();
    if !parts.next().is_some_and(|p| alpha(p, 2..=3)) {
        return false;
    }
    parts.next_if(|p| alpha(p, 4..=4));
    parts.next_if(|p| alpha(p, 2..=2) || (p.len() == 3 && p.bytes().all(|b| b.is_ascii_digit())));
    parts.all(|p| (5..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

#[cfg(test)]
pub mod test_support {
    use std::io::Write;
    use std::path::Path;

    pub const V2_MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011" xmlns:d="http://schemas.microsoft.com/developer/vsx-schema-design/2011">
  <Metadata>
    <Identity Id="MyExt.abc" Version="1.2.3" Language="en-US" Publisher="Me" />
    <DisplayName>My Extension</DisplayName>
    <Description xml:space="preserve">Does things.</Description>
    <Icon>Resources\icon.png</Icon>
    <Tags>a;b;;c</Tags>
  </Metadata>
  <Installation>
    <InstallationTarget Id="Microsoft.VisualStudio.Community" Version="[17.0, 18.0)">
      <ProductArchitecture>amd64</ProductArchitecture>
    </InstallationTarget>
    <InstallationTarget Id="Microsoft.VisualStudio.Pro" Version="[17.0,)" />
  </Installation>
</PackageManifest>"#;

    pub const V1_MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Vsix Version="1.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2010">
  <Identifier Id="Old.Ext">
    <Name>Old Ext</Name>
    <Author>Me</Author>
    <Version>2.0</Version>
    <Description>Old one.</Description>
    <Locale>1033</Locale>
    <SupportedProducts>
      <VisualStudio Version="10.0"><Edition>Pro</Edition><Edition>Ultimate</Edition></VisualStudio>
    </SupportedProducts>
  </Identifier>
</Vsix>"#;

    pub fn write_vsix(path: &Path, files: &[(&str, &[u8])]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (n, b) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn identity_from_v2_manifest() {
        let id = identity_from_manifest(V2_MANIFEST, Some("my-ext")).unwrap();
        assert_eq!(id.internal_name.as_deref(), Some("my-ext"));
        assert_eq!(id.identifier.as_deref(), Some("MyExt.abc"));
        assert_eq!(id.version.unwrap().to_string(), "1.2.3");
        assert_eq!(id.language.as_deref(), Some("en-US"));
        assert_eq!(id.display_name.as_deref(), Some("My Extension"));
        assert_eq!(id.description.as_deref(), Some("Does things."));
        assert_eq!(id.icon.as_deref(), Some("/Resources/icon.png"));
        assert_eq!(id.tags.unwrap(), vec!["a", "b", "c"]);
        let t = id.install_targets.unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].sku, VisualStudioSku::Community);
        assert_eq!(t[0].version_range.to_string(), "[17.0,18.0)");
    }

    #[test]
    fn identity_from_v1_manifest() {
        let id = identity_from_manifest(V1_MANIFEST, Some("old")).unwrap();
        assert_eq!(id.identifier.as_deref(), Some("Old.Ext"));
        assert_eq!(id.language.as_deref(), Some("en-US"));
        assert_eq!(id.display_name.as_deref(), Some("Old Ext"));
        let t = id.install_targets.unwrap();
        assert_eq!(
            t.iter().map(|t| t.sku).collect::<Vec<_>>(),
            vec![VisualStudioSku::Pro, VisualStudioSku::Ultimate]
        );
    }

    #[test]
    fn identity_errors() {
        assert!(identity_from_manifest(V2_MANIFEST, None).is_err());
        assert!(identity_from_manifest("<Other/>", Some("x")).is_err());
    }

    #[test]
    fn reads_and_extracts_parts() {
        let dir = tempfile::tempdir().unwrap();
        let vsix = dir.path().join("a.vsix");
        write_vsix(
            &vsix,
            &[
                ("extension.vsixmanifest", V2_MANIFEST.as_bytes()),
                ("Resources/icon%20x.png", b"PNG"),
            ],
        );
        let mut z = open(&vsix).unwrap();
        assert!(
            manifest_content(&mut z)
                .unwrap()
                .contains("PackageManifest")
        );
        let out = dir.path().join("icon");
        assert!(extract(&vsix, "/resources/icon x.png", &out).unwrap());
        assert_eq!(std::fs::read(&out).unwrap(), b"PNG");
        assert!(!extract(&vsix, "/missing.png", &out).unwrap());
    }

    #[test]
    fn culture_validation() {
        assert!(is_valid_culture("en-US"));
        assert!(is_valid_culture("zh-Hans"));
        assert!(is_valid_culture("1033"));
        assert!(!is_valid_culture("english"));
        assert!(is_valid_culture("es-419"));
        assert!(is_valid_culture("sr-Latn-RS"));
        assert!(is_valid_culture("de-DE-1901x"));
        assert!(!is_valid_culture("en-"));
        assert!(!is_valid_culture("e"));
        assert!(!is_valid_culture("en-USA"));
        assert!(!is_valid_culture("99999"));
    }
}
