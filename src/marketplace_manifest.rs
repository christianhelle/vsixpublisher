//! Port of `MarketplaceManifestGenerator`, emitting XML the way .NET's
//! `XDocument.Save(StringWriter)` does (utf-16 declaration, 2-space indent, CRLF).

use crate::manifest::{AssetFile, PublishManifest};

pub const NS: &str = "http://schemas.microsoft.com/developer/vsx-schema/2011";
pub const OVERVIEW_TYPE: &str = "Microsoft.VisualStudio.Services.Content.Details";
pub const ICON_TYPE: &str = "Microsoft.VisualStudio.Services.Icons.Default";
pub const PAYLOAD_TYPE: &str = "Microsoft.VisualStudio.Ide.Payload";
pub const OVERVIEW_PATH: &str = "/overview.md";
const NL: &str = "\r\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadType {
    Vsix,
    Link,
    Other,
}

/// `Path.GetFileName` with Windows separator semantics.
pub fn file_name(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

pub fn ensure_leading_slash(p: &str) -> String {
    if p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

/// `Utilities.GetPayloadName`.
pub fn payload_name(payload: &str) -> String {
    ensure_leading_slash(file_name(payload))
}

enum Content {
    Empty,
    Text(String),
    Children(Vec<El>),
}

struct El {
    name: &'static str,
    attrs: Vec<(&'static str, String)>,
    content: Content,
}

impl El {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            attrs: Vec::new(),
            content: Content::Empty,
        }
    }
    fn text(name: &'static str, t: impl Into<String>) -> Self {
        Self {
            content: Content::Text(t.into()),
            ..Self::new(name)
        }
    }
    /// `SetAttributeValue`: a `None` value removes/omits the attribute.
    fn attr(mut self, k: &'static str, v: Option<impl Into<String>>) -> Self {
        if let Some(v) = v {
            self.attrs.push((k, v.into()));
        }
        self
    }
    fn push(&mut self, child: El) {
        match &mut self.content {
            Content::Children(c) => c.push(child),
            _ => self.content = Content::Children(vec![child]),
        }
    }
    fn child_mut(&mut self, name: &'static str) -> &mut El {
        let idx = match &self.content {
            Content::Children(c) => c.iter().position(|e| e.name == name),
            _ => None,
        };
        let idx = match idx {
            Some(i) => i,
            None => {
                self.push(El::new(name));
                match &self.content {
                    Content::Children(c) => c.len() - 1,
                    _ => unreachable!(),
                }
            }
        };
        match &mut self.content {
            Content::Children(c) => &mut c[idx],
            _ => unreachable!(),
        }
    }

    fn write(&self, out: &mut String, depth: usize) {
        let indent = "  ".repeat(depth);
        out.push_str(&indent);
        out.push('<');
        out.push_str(self.name);
        for (k, v) in &self.attrs {
            out.push_str(&format!(" {k}=\"{}\"", escape_attr(v)));
        }
        if depth == 0 {
            out.push_str(&format!(" xmlns=\"{NS}\""));
        }
        match &self.content {
            Content::Empty => out.push_str(" />"),
            Content::Text(t) => {
                out.push('>');
                out.push_str(&escape_text(t));
                out.push_str(&format!("</{}>", self.name));
            }
            Content::Children(c) => {
                out.push('>');
                for e in c {
                    out.push_str(NL);
                    e.write(out, depth + 1);
                }
                out.push_str(NL);
                out.push_str(&indent);
                out.push_str(&format!("</{}>", self.name));
            }
        }
    }
}

fn escape_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\r' => o.push_str("&#xD;"),
            _ => o.push(c),
        }
    }
    o
}

fn escape_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#xA;"),
            '\r' => o.push_str("&#xD;"),
            '\t' => o.push_str("&#x9;"),
            _ => o.push(c),
        }
    }
    o
}

fn property(id: &str, value: impl Into<String>) -> El {
    El::new("Property")
        .attr("Id", Some(id))
        .attr("Value", Some(value))
}

fn asset_tag(a: &AssetFile) -> El {
    let path = a.target_path.trim_start_matches('/').to_string();
    let ty = a
        .asset_type
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| path.clone());
    El::new("Asset")
        .attr("Type", Some(ty))
        .attr("Path", Some(path))
        .attr("Addressable", Some("true"))
}

/// Generates `extension.marketplacemanifest`. Identity must be resolved.
pub fn generate(payload: &str, payload_type: PayloadType, m: &PublishManifest) -> String {
    let id = m.identity.as_ref().expect("identity");
    let mut root = El::new("PackageManifest").attr("Version", Some("2.0.0"));

    // Metadata
    let md = root.child_mut("Metadata");
    md.push(
        El::new("Identity")
            .attr("Language", id.language.clone())
            .attr("Id", id.internal_name.clone())
            .attr("Version", id.version.map(|v| v.to_string()))
            .attr("Publisher", m.publisher.clone()),
    );
    md.push(El::text(
        "DisplayName",
        id.display_name.clone().unwrap_or_default(),
    ));
    md.push(
        El::text("Description", id.description.clone().unwrap_or_default())
            .attr("xml:space", Some("preserve")),
    );
    if let Some(c) = m.categories.as_ref().filter(|c| !c.is_empty()) {
        let names: Vec<&str> = c.iter().map(|c| c.server_name()).collect();
        md.push(El::text("Categories", names.join(",")));
    }
    if let Some(w) = m.ignore_warnings.as_ref().filter(|w| !w.is_empty()) {
        md.push(El::text("IgnoreWarnings", w.join(",")));
    }
    if let Some(vsix_id) = id.identifier.as_ref().filter(|s| !s.is_empty()) {
        md.push(El::text("VsixId", vsix_id.clone()));
    }
    if let Some(t) = id.tags.as_ref().filter(|t| !t.is_empty()) {
        md.push(El::text("Tags", t.join(",")));
    }
    let mut flags = vec![m.price_category.as_str()];
    if !m.is_private {
        flags.push("Public");
    }
    md.push(El::text("GalleryFlags", flags.join(" ")));
    if let Some(icon) = id.icon.as_ref().filter(|s| !s.is_empty()) {
        md.push(El::text("Icon", ensure_leading_slash(file_name(icon))));
    }
    let mut props = El::new("Properties");
    props.push(property(
        "Microsoft.VisualStudio.Services.EnableMarketplaceQnA",
        if m.qna { "true" } else { "false" },
    ));
    if let Some(repo) = m.repo.as_ref().filter(|s| !s.is_empty()) {
        props.push(property(
            "Microsoft.VisualStudio.Services.Links.Source",
            repo.clone(),
        ));
    }
    md.push(props);

    // Installation
    if payload_type != PayloadType::Vsix {
        let inst = root.child_mut("Installation");
        inst.push(
            El::new("InstallationTarget")
                .attr("Id", Some("Microsoft.VisualStudio.Ide"))
                .attr("Version", Some("[0.0,0.0]")),
        );
        for t in id.install_targets.iter().flatten() {
            inst.push(
                El::new("InstallationTarget")
                    .attr("Id", Some(t.sku.fully_qualified_name()))
                    .attr("Version", Some(t.version_range.to_string())),
            );
        }
    }

    // Assets
    let assets = root.child_mut("Assets");
    for a in &m.asset_files {
        assets.push(asset_tag(a));
    }

    // Payload
    if payload_type == PayloadType::Link {
        root.child_mut("Metadata")
            .child_mut("Properties")
            .push(property(
                "Microsoft.VisualStudio.Services.Links.Getstarted",
                payload,
            ));
    } else {
        root.child_mut("Assets").push(
            El::new("Asset")
                .attr("Type", Some(PAYLOAD_TYPE))
                .attr("Path", Some(payload_name(payload)))
                .attr("Addressable", Some("true")),
        );
    }

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-16\" standalone=\"yes\"?>");
    out.push_str(NL);
    root.write(&mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{Category, PriceCategory, VisualStudioSku};
    use crate::manifest::{Identity, InstallTarget};
    use crate::version::{Version, VersionRange};

    fn manifest() -> PublishManifest {
        PublishManifest {
            publisher: Some("me".into()),
            overview: Some("README.md".into()),
            categories: Some(vec![Category::Coding, Category::Other]),
            repo: Some("https://github.com/me/x".into()),
            identity: Some(Identity {
                internal_name: Some("my-ext".into()),
                identifier: Some("MyExt.abc".into()),
                version: Version::parse("1.2.3"),
                language: Some("en-US".into()),
                display_name: Some("My <Ext>".into()),
                description: Some("Does & things".into()),
                icon: Some("C:\\temp\\tmp123.tmp".into()),
                tags: Some(vec!["a".into(), "b".into()]),
                install_targets: Some(vec![InstallTarget {
                    version_range: VersionRange::parse("[17.0,18.0)").unwrap(),
                    sku: VisualStudioSku::Community,
                }]),
            }),
            asset_files: vec![
                AssetFile {
                    path_on_disk: "README.md".into(),
                    target_path: "/overview.md".into(),
                    asset_type: Some(OVERVIEW_TYPE.into()),
                },
                AssetFile {
                    path_on_disk: "img/a.png".into(),
                    target_path: "img/a.png".into(),
                    asset_type: None,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn vsix_payload_golden() {
        let xml = generate("C:\\out\\My.vsix", PayloadType::Vsix, &manifest());
        let expected = [
            r#"<?xml version="1.0" encoding="utf-16" standalone="yes"?>"#,
            r#"<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">"#,
            r#"  <Metadata>"#,
            r#"    <Identity Language="en-US" Id="my-ext" Version="1.2.3" Publisher="me" />"#,
            r#"    <DisplayName>My &lt;Ext&gt;</DisplayName>"#,
            r#"    <Description xml:space="preserve">Does &amp; things</Description>"#,
            r#"    <Categories>coding,othertools</Categories>"#,
            r#"    <VsixId>MyExt.abc</VsixId>"#,
            r#"    <Tags>a,b</Tags>"#,
            r#"    <GalleryFlags>Free Public</GalleryFlags>"#,
            r#"    <Icon>/tmp123.tmp</Icon>"#,
            r#"    <Properties>"#,
            r#"      <Property Id="Microsoft.VisualStudio.Services.EnableMarketplaceQnA" Value="true" />"#,
            r#"      <Property Id="Microsoft.VisualStudio.Services.Links.Source" Value="https://github.com/me/x" />"#,
            r#"    </Properties>"#,
            r#"  </Metadata>"#,
            r#"  <Assets>"#,
            r#"    <Asset Type="Microsoft.VisualStudio.Services.Content.Details" Path="overview.md" Addressable="true" />"#,
            r#"    <Asset Type="img/a.png" Path="img/a.png" Addressable="true" />"#,
            r#"    <Asset Type="Microsoft.VisualStudio.Ide.Payload" Path="/My.vsix" Addressable="true" />"#,
            r#"  </Assets>"#,
            r#"</PackageManifest>"#,
        ]
        .join("\r\n");
        assert_eq!(xml, expected);
    }

    #[test]
    fn link_payload_adds_installation_and_getstarted() {
        let mut m = manifest();
        m.is_private = true;
        m.qna = false;
        m.price_category = PriceCategory::Paid;
        m.ignore_warnings = Some(vec!["W1".into(), "W2".into()]);
        let xml = generate("https://example.com/dl", PayloadType::Link, &m);
        assert!(xml.contains("<IgnoreWarnings>W1,W2</IgnoreWarnings>"));
        assert!(xml.contains("<GalleryFlags>Paid</GalleryFlags>"));
        assert!(xml.contains(r#"Value="false" />"#));
        assert!(xml.contains(
            "  <Installation>\r\n    <InstallationTarget Id=\"Microsoft.VisualStudio.Ide\" Version=\"[0.0,0.0]\" />"
        ));
        assert!(xml.contains(
            r#"<InstallationTarget Id="Microsoft.VisualStudio.Community" Version="[17.0,18.0)" />"#
        ));
        assert!(xml.contains(r#"<Property Id="Microsoft.VisualStudio.Services.Links.Getstarted" Value="https://example.com/dl" />"#));
        assert!(!xml.contains("Microsoft.VisualStudio.Ide.Payload"));
        // Installation comes between Metadata and Assets.
        assert!(xml.find("</Metadata>").unwrap() < xml.find("<Installation>").unwrap());
        assert!(xml.find("</Installation>").unwrap() < xml.find("<Assets>").unwrap());
    }

    #[test]
    fn other_payload_has_installation_and_payload_asset() {
        let xml = generate("/tmp/setup.exe", PayloadType::Other, &manifest());
        assert!(xml.contains("<Installation>"));
        assert!(xml.contains(r#"Path="/setup.exe""#));
    }

    #[test]
    fn file_name_handles_both_separators() {
        assert_eq!(file_name("a/b\\c.vsix"), "c.vsix");
        assert_eq!(payload_name("x.vsix"), "/x.vsix");
    }
}
