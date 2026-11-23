//! Minimal namespace-aware XML DOM (enough for `XDocument`-style lookups).

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

#[derive(Debug, Clone, Default)]
pub struct Element {
    pub ns: String,
    pub name: String,
    /// Attributes without a namespace prefix: (local name, value).
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn is(&self, ns: &str, name: &str) -> bool {
        self.ns == ns && self.name == name
    }

    pub fn element<'a>(&'a self, ns: &'a str, name: &'a str) -> Option<&'a Element> {
        self.elements(ns, name).next()
    }

    pub fn elements<'a>(&'a self, ns: &'a str, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter_map(move |c| match c {
            Node::Element(e) if e.is(ns, name) => Some(e),
            _ => None,
        })
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// `XElement.Value`: concatenated descendant text.
    pub fn value(&self) -> String {
        let mut s = String::new();
        self.collect_text(&mut s);
        s
    }

    fn collect_text(&self, s: &mut String) {
        for c in &self.children {
            match c {
                Node::Text(t) => s.push_str(t),
                Node::Element(e) => e.collect_text(s),
            }
        }
    }
}

fn ns_of(r: ResolveResult) -> String {
    match r {
        ResolveResult::Bound(ns) => String::from_utf8_lossy(ns.as_ref()).into_owned(),
        _ => String::new(),
    }
}

pub fn parse(text: &str) -> Result<Element, String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut reader = NsReader::from_str(text);
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    let start = |reader: &NsReader<&[u8]>, e: &quick_xml::events::BytesStart| -> Result<Element, String> {
        let (ns, local) = reader.resolve_element(e.name());
        let mut el = Element {
            ns: ns_of(ns),
            name: String::from_utf8_lossy(local.as_ref()).into_owned(),
            ..Default::default()
        };
        for a in e.attributes() {
            let a = a.map_err(|e| e.to_string())?;
            let key = a.key;
            if key.as_ref().starts_with(b"xmlns") || key.prefix().is_some() {
                continue;
            }
            let v = a.unescape_value().map_err(|e| e.to_string())?.into_owned();
            el.attrs.push((String::from_utf8_lossy(key.as_ref()).into_owned(), v));
        }
        Ok(el)
    };

    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let el = start(&reader, &e)?;
                stack.push(el);
            }
            Event::Empty(e) => {
                let el = start(&reader, &e)?;
                match stack.last_mut() {
                    Some(p) => p.children.push(Node::Element(el)),
                    None => root = Some(el),
                }
            }
            Event::End(_) => {
                let el = stack.pop().ok_or("unbalanced end tag")?;
                match stack.last_mut() {
                    Some(p) => p.children.push(Node::Element(el)),
                    None => root = Some(el),
                }
            }
            Event::Text(t) => {
                if let Some(p) = stack.last_mut() {
                    p.children.push(Node::Text(t.unescape().map_err(|e| e.to_string())?.into_owned()));
                }
            }
            Event::CData(t) => {
                if let Some(p) = stack.last_mut() {
                    p.children.push(Node::Text(String::from_utf8_lossy(&t).into_owned()));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| "Root element is missing.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaces_attrs_and_text() {
        let doc = parse(
            r#"<?xml version="1.0"?><Root xmlns="urn:a" xmlns:d="urn:d"><Item Id="1" d:x="y">Hi &amp; <b>there</b></Item><Item Id="2"/></Root>"#,
        )
        .unwrap();
        assert!(doc.is("urn:a", "Root"));
        let items: Vec<_> = doc.elements("urn:a", "Item").collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].attr("Id"), Some("1"));
        assert_eq!(items[0].attr("x"), None);
        assert_eq!(items[0].value(), "Hi & there");
        assert!(doc.element("urn:other", "Item").is_none());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("").is_err());
        assert!(parse("<a><b></a>").is_err());
    }
}
