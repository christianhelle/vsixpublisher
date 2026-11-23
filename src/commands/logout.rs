//! Port of `LogoutCommand`.

use std::path::PathBuf;

use crate::cli::{Args, Argument, ArgumentType::*, Command};
use crate::db::PublisherDb;
use crate::logger::Logger;
use crate::messages as msg;

pub struct LogoutCommand {
    pub db_path: PathBuf,
}

impl Command for LogoutCommand {
    fn arguments(&self) -> Vec<Argument> {
        vec![
            Argument::new(
                "publisherName",
                NonBoolean,
                msg::LOGOUT_ARGUMENT_PUBLISHER_NAME,
            )
            .required(),
            Argument::new(
                "ignoreMissingPublisher",
                Boolean,
                msg::LOGOUT_ARGUMENT_IGNORE_MISSING_PUBLISHER,
            ),
        ]
    }

    fn help_text(&self) -> &'static str {
        msg::LOGOUT_HELP_TEXT
    }

    fn validate(&self, args: &Args, log: &mut Logger) -> i32 {
        if args.get("publisherName").unwrap_or_default().is_empty() {
            return log.error(msg::MISSING_PUBLISHER_NAME, &[&"logout"]);
        }
        0
    }

    fn execute(&self, args: &Args, log: &mut Logger) -> i32 {
        let name = args.get("publisherName").unwrap_or_default();
        let db = PublisherDb::new(&self.db_path);
        db.remove(name, args.flag("ignoreMissingPublisher"), log);
        if let Some(e) = log.last_error() {
            return e.error_code();
        }
        log.info(msg::INFO_PUBLISHER_LOGGED_OUT, &[&name]);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ArgValue;
    use crate::logger::test_support::capture;

    fn args(name: &str, ignore: bool) -> Args {
        let mut a = Args::default();
        a.insert("publisherName", ArgValue::Value(name.into()));
        if ignore {
            a.insert("ignoreMissingPublisher", ArgValue::Flag);
        }
        a
    }

    #[test]
    fn logs_out_known_publisher() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".vsce");
        let (mut log, out, _) = capture();
        PublisherDb::new(&path).add("me", "pat", false, &mut log);
        let cmd = LogoutCommand {
            db_path: path.clone(),
        };
        assert_eq!(cmd.execute(&args("me", false), &mut log), 0);
        assert_eq!(
            out.text(),
            "VSSDK: information VsixPub0042 : Publisher 'me' has been logged-out.\n"
        );
        assert!(!path.exists());
    }

    #[test]
    fn unknown_publisher_fails_unless_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = LogoutCommand {
            db_path: dir.path().join(".vsce"),
        };
        let (mut log, _, _) = capture();
        assert_eq!(cmd.execute(&args("me", true), &mut log), 0);
        assert_eq!(cmd.execute(&args("me", false), &mut log), 14);
    }
}
