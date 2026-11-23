//! Port of `Microsoft.VisualStudio.Sdk.Logger` / `OutputMessage`.

use std::fmt::Display;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Error,
    Information,
}

impl MessageType {
    fn as_lower(self) -> &'static str {
        match self {
            MessageType::Error => "error",
            MessageType::Information => "information",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputMessage {
    pub text: &'static str,
    pub id: &'static str,
    pub kind: MessageType,
}

impl OutputMessage {
    pub const fn new(text: &'static str, id: &'static str, kind: MessageType) -> Self {
        Self { text, id, kind }
    }

    /// Numeric part of the id, e.g. `VsixPub0007` -> 7.
    pub fn error_code(&self) -> i32 {
        let digits: String = self
            .id
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().unwrap_or(-1)
    }

    pub fn output_text(&self, args: &[&dyn Display]) -> String {
        format!(
            "VSSDK: {} {} : {}",
            self.kind.as_lower(),
            self.id.trim_end(),
            format_net(self.text, args)
        )
    }
}

/// Minimal `string.Format`: replaces `{n}` placeholders.
pub fn format_net(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut idx = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    idx.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            if !idx.is_empty() && chars.peek() == Some(&'}') {
                chars.next();
                let i: usize = idx.parse().unwrap();
                if let Some(a) = args.get(i) {
                    out.push_str(&a.to_string());
                }
                continue;
            }
            out.push('{');
            out.push_str(&idx);
            continue;
        }
        out.push(c);
    }
    out
}

pub struct Logger {
    out: Box<dyn Write>,
    err: Box<dyn Write>,
    last_error: Option<OutputMessage>,
}

impl Logger {
    pub fn new(out: Box<dyn Write>, err: Box<dyn Write>) -> Self {
        Self {
            out,
            err,
            last_error: None,
        }
    }

    pub fn stdio() -> Self {
        Self::new(Box::new(std::io::stdout()), Box::new(std::io::stderr()))
    }

    pub fn last_error(&self) -> Option<OutputMessage> {
        self.last_error
    }

    /// Error code of the last error, or `default` when none was logged.
    pub fn last_error_code_or(&self, default: i32) -> i32 {
        self.last_error.map(|m| m.error_code()).unwrap_or(default)
    }

    pub fn error(&mut self, msg: OutputMessage, args: &[&dyn Display]) -> i32 {
        self.last_error = Some(msg);
        let _ = writeln!(self.err, "{}", msg.output_text(args));
        msg.error_code()
    }

    pub fn info(&mut self, msg: OutputMessage, args: &[&dyn Display]) {
        let _ = writeln!(self.out, "{}", msg.output_text(args));
    }

    pub fn message(&mut self, text: &str) {
        let _ = writeln!(self.out, "{text}");
    }
}

#[cfg(test)]
pub mod test_support {
    use super::Logger;
    use std::cell::RefCell;
    use std::io::Write;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    pub struct Buf(pub Rc<RefCell<Vec<u8>>>);

    impl Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Buf {
        pub fn text(&self) -> String {
            String::from_utf8(self.0.borrow().clone()).unwrap()
        }
    }

    pub fn capture() -> (Logger, Buf, Buf) {
        let out = Buf::default();
        let err = Buf::default();
        (
            Logger::new(Box::new(out.clone()), Box::new(err.clone())),
            out,
            err,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERR: OutputMessage = OutputMessage::new(
        "Cannot locate '{0}' ({1}).",
        "VsixPub0007",
        MessageType::Error,
    );
    const INFO: OutputMessage =
        OutputMessage::new("Uploaded '{0}'.", "VsixPub0038", MessageType::Information);

    #[test]
    fn error_code_from_id() {
        assert_eq!(ERR.error_code(), 7);
        assert_eq!(INFO.error_code(), 38);
    }

    #[test]
    fn output_text_format() {
        assert_eq!(
            ERR.output_text(&[&"x", &3]),
            "VSSDK: error VsixPub0007 : Cannot locate 'x' (3)."
        );
        assert_eq!(
            INFO.output_text(&[&"Ext"]),
            "VSSDK: information VsixPub0038 : Uploaded 'Ext'."
        );
    }

    #[test]
    fn format_leaves_non_placeholders() {
        assert_eq!(format_net("a {x} {0} {", &[&1]), "a {x} 1 {");
    }

    #[test]
    fn logger_routes_streams_and_tracks_last_error() {
        let (mut log, out, err) = test_support::capture();
        assert_eq!(log.last_error_code_or(0), 0);
        log.info(INFO, &[&"E"]);
        assert_eq!(log.error(ERR, &[&"p", &1]), 7);
        assert_eq!(
            out.text(),
            "VSSDK: information VsixPub0038 : Uploaded 'E'.\n"
        );
        assert!(err.text().contains("VsixPub0007"));
        assert_eq!(log.last_error(), Some(ERR));
    }
}
