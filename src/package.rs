//! Port of `PayloadPackager`: builds the OPC (zip) upload package in memory.

use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::logger::Logger;
use crate::manifest::PublishManifest;
use crate::marketplace_manifest::{ensure_leading_slash, file_name, payload_name};
use crate::messages as msg;
use crate::validate::resolve;

const CONTENT_TYPES_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const MANIFEST_PART: &str = "/extension.marketplacemanifest";

/// `Utilities.GetMimeTypeFromExtension` (extension includes the dot).
pub fn mime_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        ".json" | ".vsext" => "application/json",
        ".txt" | ".pkgdef" => "text/plain",
        ".xml" | ".vsixmanifest" => "text/xml",
        ".html" | ".htm" => "text/html",
        ".pdf" => "application/pdf",
        ".rtf" => "text/richtext",
        ".gif" => "image/gif",
        ".jpeg" | ".jpg" => "image/jpeg",
        ".tiff" => "image/tiff",
        ".vsix" | ".zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// `Path.GetExtension` (with dot, empty when none).
pub fn extension(p: &str) -> &str {
    let name = file_name(p);
    match name.rfind('.') {
        Some(i) if i + 1 < name.len() => &name[i..],
        _ => "",
    }
}

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&b)
}

/// Validates and escapes an OPC part name (`/seg/seg.ext`). Returns the escaped name.
pub fn part_name(raw: &str) -> Result<String, String> {
    if !raw.starts_with('/') || raw.ends_with('/') || raw.contains('\\') || raw.contains("//") {
        return Err(format!("Invalid part name '{raw}'."));
    }
    let mut out = String::new();
    for seg in raw[1..].split('/') {
        if seg.is_empty() || seg.ends_with('.') || seg == "." || seg == ".." {
            return Err(format!("Invalid part name '{raw}'."));
        }
        out.push('/');
        let bytes = seg.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'%' {
                let hex = seg
                    .get(i + 1..i + 3)
                    .ok_or_else(|| format!("Invalid part name '{raw}'."))?;
                let v = u8::from_str_radix(hex, 16)
                    .map_err(|_| format!("Invalid part name '{raw}'."))?;
                if v == b'/' || v == b'\\' {
                    return Err(format!("Invalid part name '{raw}'."));
                }
                out.push_str(&seg[i..i + 3]);
                i += 3;
                continue;
            }
            if is_unreserved(b) {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
            i += 1;
        }
    }
    Ok(out)
}

struct Builder {
    zip: ZipWriter<Cursor<Vec<u8>>>,
    defaults: BTreeMap<String, &'static str>,
    default_order: Vec<String>,
    overrides: Vec<(String, &'static str)>,
    names: Vec<String>,
}

impl Builder {
    fn new() -> Self {
        Self {
            zip: ZipWriter::new(Cursor::new(Vec::new())),
            defaults: BTreeMap::new(),
            default_order: Vec::new(),
            overrides: Vec::new(),
            names: Vec::new(),
        }
    }

    fn add(&mut self, part: &str, content_type: &'static str, data: &[u8]) -> Result<(), String> {
        let lower = part.to_lowercase();
        if self.names.contains(&lower) {
            return Err(format!(
                "Cannot add part for the specified URI because it is already in the package: '{part}'."
            ));
        }
        self.names.push(lower);
        let ext = extension(part).trim_start_matches('.').to_lowercase();
        if ext.is_empty() {
            self.overrides.push((part.to_string(), content_type));
        } else {
            match self.defaults.get(&ext) {
                None => {
                    self.defaults.insert(ext.clone(), content_type);
                    self.default_order.push(ext);
                }
                Some(t) if *t == content_type => {}
                Some(_) => self.overrides.push((part.to_string(), content_type)),
            }
        }
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        self.zip
            .start_file(&part[1..], opts)
            .map_err(|e| e.to_string())?;
        self.zip.write_all(data).map_err(|e| e.to_string())
    }

    fn finish(mut self) -> Result<Vec<u8>, String> {
        // System.IO.Packaging writes a UTF-8 BOM.
        let mut ct = format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?><Types xmlns=\"{CONTENT_TYPES_NS}\">"
        );
        for ext in &self.default_order {
            ct.push_str(&format!(
                "<Default Extension=\"{ext}\" ContentType=\"{}\" />",
                self.defaults[ext]
            ));
        }
        for (p, t) in &self.overrides {
            ct.push_str(&format!(
                "<Override PartName=\"{p}\" ContentType=\"{t}\" />"
            ));
        }
        ct.push_str("</Types>");
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        self.zip
            .start_file("[Content_Types].xml", opts)
            .map_err(|e| e.to_string())?;
        self.zip
            .write_all(ct.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(self.zip.finish().map_err(|e| e.to_string())?.into_inner())
    }
}

/// Builds the upload package. `payload_path` is `None` for link payloads.
/// Logs and returns `None` on failure.
pub fn build(
    m: &PublishManifest,
    marketplace_manifest: &str,
    payload_path: Option<&Path>,
    base_path: &Path,
    log: &mut Logger,
) -> Option<Vec<u8>> {
    let mut b = Builder::new();
    if let Some(payload) = payload_path {
        let display = payload.to_string_lossy();
        let name = match part_name(&payload_name(&display)) {
            Ok(n) => n,
            Err(_) => {
                log.error(msg::PAYLOAD_FILENAME_INVALID, &[&file_name(&display)]);
                return None;
            }
        };
        let data = match std::fs::read(payload) {
            Ok(d) => d,
            Err(e) => {
                log.error(msg::GENERIC_ERROR_PUBLISHING, &[&display, &e]);
                return None;
            }
        };
        if let Err(e) = b.add(&name, mime_type(extension(&display)), &data) {
            log.error(msg::GENERIC_ERROR_PUBLISHING, &[&display, &e]);
            return None;
        }
    }
    if let Err(e) = b.add(MANIFEST_PART, "text/xml", marketplace_manifest.as_bytes()) {
        log.error(msg::GENERIC_ERROR_PUBLISHING, &[&MANIFEST_PART, &e]);
        return None;
    }
    for a in &m.asset_files {
        let disk = resolve(base_path, &a.path_on_disk);
        let disk_s = disk.to_string_lossy().into_owned();
        let name = match part_name(&ensure_leading_slash(&a.target_path)) {
            Ok(n) => n,
            Err(_) => {
                log.error(msg::INVALID_URI, &[&a.target_path, &"AssetFile"]);
                return None;
            }
        };
        let data = match std::fs::read(&disk) {
            Ok(d) => d,
            Err(e) => {
                log.error(msg::GENERIC_ERROR_PUBLISHING, &[&disk_s, &e]);
                return None;
            }
        };
        if let Err(e) = b.add(&name, mime_type(extension(&disk_s)), &data) {
            log.error(msg::INVALID_URI, &[&a.target_path, &e]);
            return None;
        }
    }
    match b.finish() {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            log.error(msg::GENERIC_ERROR_PUBLISHING, &[&"package", &e]);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::test_support::capture;
    use crate::manifest::AssetFile;
    use std::io::Read;

    fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        (0..z.len())
            .map(|i| {
                let mut f = z.by_index(i).unwrap();
                let mut b = Vec::new();
                f.read_to_end(&mut b).unwrap();
                (f.name().unwrap().to_string(), b)
            })
            .collect()
    }

    #[test]
    fn mime_types() {
        assert_eq!(mime_type(".VSIX"), "application/zip");
        assert_eq!(mime_type(".md"), "application/octet-stream");
        assert_eq!(mime_type(""), "application/octet-stream");
        assert_eq!(extension("a/b.c/d"), "");
        assert_eq!(extension("x.tar.gz"), ".gz");
    }

    #[test]
    fn part_names() {
        assert_eq!(part_name("/My Ext.vsix").unwrap(), "/My%20Ext.vsix");
        assert_eq!(part_name("/img/a.png").unwrap(), "/img/a.png");
        assert!(part_name("/a/").is_err());
        assert!(part_name("/a//b").is_err());
        assert!(part_name("/a./b").is_err());
        assert!(part_name("/a%2Fb").is_err());
    }

    #[test]
    fn builds_package_with_content_types() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("My.vsix"), b"VSIX").unwrap();
        std::fs::write(d.path().join("README.md"), b"# hi").unwrap();
        std::fs::write(d.path().join("a.png"), b"PNG").unwrap();
        let m = PublishManifest {
            asset_files: vec![
                AssetFile {
                    path_on_disk: "README.md".into(),
                    target_path: "/overview.md".into(),
                    asset_type: None,
                },
                AssetFile {
                    path_on_disk: "a.png".into(),
                    target_path: "img/a.png".into(),
                    asset_type: None,
                },
            ],
            ..Default::default()
        };
        let (mut log, _, _) = capture();
        let pkg = build(
            &m,
            "<x/>",
            Some(&d.path().join("My.vsix")),
            d.path(),
            &mut log,
        )
        .unwrap();
        let e = entries(&pkg);
        let names: Vec<&str> = e.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "My.vsix",
                "extension.marketplacemanifest",
                "overview.md",
                "img/a.png",
                "[Content_Types].xml"
            ]
        );
        assert_eq!(e[0].1, b"VSIX");
        assert_eq!(e[1].1, b"<x/>");
        let ct = String::from_utf8(e[4].1.clone()).unwrap();
        assert_eq!(
            ct,
            "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"vsix\" ContentType=\"application/zip\" />\
<Default Extension=\"marketplacemanifest\" ContentType=\"text/xml\" />\
<Default Extension=\"md\" ContentType=\"application/octet-stream\" />\
<Default Extension=\"png\" ContentType=\"application/octet-stream\" /></Types>"
        );
    }

    #[test]
    fn link_payload_has_no_payload_part_and_bad_asset_fails() {
        let d = tempfile::tempdir().unwrap();
        let (mut log, _, _) = capture();
        let pkg = build(
            &PublishManifest::default(),
            "<x/>",
            None,
            d.path(),
            &mut log,
        )
        .unwrap();
        assert_eq!(entries(&pkg).len(), 2);
        let m = PublishManifest {
            asset_files: vec![AssetFile {
                path_on_disk: "x".into(),
                target_path: "bad/".into(),
                asset_type: None,
            }],
            ..Default::default()
        };
        assert!(build(&m, "<x/>", None, d.path(), &mut log).is_none());
        assert_eq!(log.last_error_code_or(0), 34);
    }
}
