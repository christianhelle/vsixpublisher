//! Port of `LoginCommand`.

use std::path::PathBuf;

use crate::cli::{Args, Argument, ArgumentType::*, Command};
use crate::client::{parse_marketplace_url, ClientFactory};
use crate::db::PublisherDb;
use crate::logger::Logger;
use crate::messages as msg;

pub struct LoginCommand {
    pub db_path: PathBuf,
    pub factory: Box<dyn ClientFactory>,
}

impl Command for LoginCommand {
    fn arguments(&self) -> Vec<Argument> {
        vec![
            Argument::new("publisherName", NonBoolean, msg::LOGIN_ARGUMENT_PUBLISHER_NAME).required(),
            Argument::new("personalAccessToken", NonBoolean, msg::ARGUMENT_PERSONAL_ACCESS_TOKEN).required(),
            Argument::new("overwrite", Boolean, msg::LOGIN_ARGUMENT_OVERWRITE),
            Argument::new("marketplaceUrl", NonBoolean, msg::ARGUMENT_MARKETPLACE_URL).hidden(),
        ]
    }

    fn help_text(&self) -> &'static str {
        msg::LOGIN_HELP_TEXT
    }

    fn validate(&self, args: &Args, log: &mut Logger) -> i32 {
        if args.get("publisherName").unwrap_or_default().is_empty() {
            return log.error(msg::MISSING_PUBLISHER_NAME, &[&"login"]);
        }
        if args.get("personalAccessToken").unwrap_or_default().is_empty() {
            return log.error(msg::MISSING_PAT_IN_LOGIN, &[]);
        }
        0
    }

    fn execute(&self, args: &Args, log: &mut Logger) -> i32 {
        let name = args.get("publisherName").unwrap_or_default();
        let pat = args.get("personalAccessToken").unwrap_or_default();
        let Some(url) = parse_marketplace_url(args.get("marketplaceUrl"), log) else {
            return log.last_error_code_or(-1);
        };
        let client = match super::connect(self.factory.as_ref(), pat, &url, log) {
            Ok(c) => c,
            Err(code) => return code,
        };
        if client.get_publisher(name, log).is_none() {
            return log.error(msg::PUBLISHER_NOT_FOUND_IN_MARKETPLACE, &[&name]);
        }
        PublisherDb::new(&self.db_path).add(name, pat, args.flag("overwrite"), log);
        if let Some(e) = log.last_error() {
            return e.error_code();
        }
        log.info(msg::INFO_PUBLISHER_LOGGED_IN, &[&name]);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::MarketplacePublisher;
    use crate::commands::fake::{args, Fake};
    use crate::logger::test_support::capture;

    fn cmd(fake: &Fake, dir: &tempfile::TempDir) -> LoginCommand {
        LoginCommand { db_path: dir.path().join(".vsce"), factory: Box::new(fake.clone()) }
    }

    fn login_args() -> Args {
        args(&[("publisherName", Some("me")), ("personalAccessToken", Some("pat"))])
    }

    #[test]
    fn stores_pat_when_publisher_exists() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::default();
        fake.0.borrow_mut().publisher = Some(MarketplacePublisher::default());
        let (mut log, out, _) = capture();
        assert_eq!(cmd(&fake, &dir).execute(&login_args(), &mut log), 0);
        assert_eq!(out.text(), "VSSDK: information VsixPub0041 : Publisher 'me' is now logged-in.\n");
        assert_eq!(fake.0.borrow().pat.as_deref(), Some("pat"));
        let db = PublisherDb::new(dir.path().join(".vsce"));
        assert_eq!(db.personal_access_token("me", &mut log).as_deref(), Some("pat"));
        // Second login without -overwrite fails.
        assert_eq!(cmd(&fake, &dir).execute(&login_args(), &mut log), 12);
    }

    #[test]
    fn unknown_publisher() {
        let dir = tempfile::tempdir().unwrap();
        let (mut log, _, _) = capture();
        assert_eq!(cmd(&Fake::default(), &dir).execute(&login_args(), &mut log), 30);
    }

    #[test]
    fn auth_failure_returns_its_code() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::default();
        fake.0.borrow_mut().auth_fails = true;
        let (mut log, _, _) = capture();
        assert_eq!(cmd(&fake, &dir).execute(&login_args(), &mut log), 31);
    }

    #[test]
    fn validate_empty_values() {
        let dir = tempfile::tempdir().unwrap();
        let (mut log, _, _) = capture();
        let c = cmd(&Fake::default(), &dir);
        let a = args(&[("publisherName", Some("")), ("personalAccessToken", Some("p"))]);
        assert_eq!(c.validate(&a, &mut log), 10);
        let a = args(&[("publisherName", Some("a")), ("personalAccessToken", Some(""))]);
        assert_eq!(c.validate(&a, &mut log), 11);
    }
}
