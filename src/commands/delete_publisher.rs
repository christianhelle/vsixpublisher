//! Port of `DeletePublisherCommand`.

use std::path::PathBuf;

use crate::cli::{Args, Argument, ArgumentType::*, Command};
use crate::client::{ClientFactory, parse_marketplace_url};
use crate::db::PublisherDb;
use crate::logger::Logger;
use crate::messages as msg;

pub struct DeletePublisherCommand {
    pub db_path: PathBuf,
    pub factory: Box<dyn ClientFactory>,
}

impl Command for DeletePublisherCommand {
    fn arguments(&self) -> Vec<Argument> {
        vec![
            Argument::new(
                "publisherName",
                NonBoolean,
                msg::DELETE_PUBLISHER_ARGUMENT_PUBLISHER_NAME,
            )
            .required(),
            Argument::new(
                "personalAccessToken",
                NonBoolean,
                msg::ARGUMENT_PERSONAL_ACCESS_TOKEN,
            )
            .required(),
            Argument::new("marketplaceUrl", NonBoolean, msg::ARGUMENT_MARKETPLACE_URL).hidden(),
        ]
    }

    fn help_text(&self) -> &'static str {
        msg::DELETE_PUBLISHER_HELP_TEXT
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
        if client.delete_publisher(name, log) {
            PublisherDb::new(&self.db_path).remove(name, true, log);
        }
        if let Some(e) = log.last_error() {
            return e.error_code();
        }
        log.info(msg::INFO_PUBLISHER_DELETED, &[&name]);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::fake::{Fake, args};
    use crate::logger::test_support::capture;

    #[test]
    fn deletes_and_logs_out() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join(".vsce");
        let (mut log, out, _) = capture();
        PublisherDb::new(&db).add("me", "p", false, &mut log);
        let fake = Fake::default();
        fake.0.borrow_mut().delete_publisher_ok = true;
        let cmd = DeletePublisherCommand {
            db_path: db.clone(),
            factory: Box::new(fake.clone()),
        };
        let a = args(&[
            ("publisherName", Some("me")),
            ("personalAccessToken", Some("p")),
        ]);
        assert_eq!(cmd.execute(&a, &mut log), 0);
        assert!(!db.exists());
        assert!(out.text().ends_with(
            "VSSDK: information VsixPub0040 : Removed 'me' as a publisher on the marketplace.\n"
        ));
    }
}
