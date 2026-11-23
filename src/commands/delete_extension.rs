//! Port of `DeleteExtensionCommand`.

use std::path::PathBuf;

use crate::cli::{Args, Argument, ArgumentType::*, Command};
use crate::client::{parse_marketplace_url, ClientFactory, DeleteResult};
use crate::db::PublisherDb;
use crate::logger::Logger;
use crate::messages as msg;

pub struct DeleteExtensionCommand {
    pub db_path: PathBuf,
    pub factory: Box<dyn ClientFactory>,
}

impl Command for DeleteExtensionCommand {
    fn arguments(&self) -> Vec<Argument> {
        vec![
            Argument::new("publisherName", NonBoolean, msg::DELETE_EXTENSION_ARGUMENT_PUBLISHER_NAME).required(),
            Argument::new("extensionName", NonBoolean, msg::DELETE_EXTENSION_ARGUMENT_EXTENSION_NAME).required(),
            Argument::new("personalAccessToken", NonBoolean, msg::ARGUMENT_PERSONAL_ACCESS_TOKEN),
            Argument::new("marketplaceUrl", NonBoolean, msg::ARGUMENT_MARKETPLACE_URL).hidden(),
        ]
    }

    fn help_text(&self) -> &'static str {
        msg::DELETE_EXTENSION_HELP_TEXT
    }

    fn execute(&self, args: &Args, log: &mut Logger) -> i32 {
        let publisher = args.get("publisherName").unwrap_or_default();
        let extension = args.get("extensionName").unwrap_or_default();
        let Some(url) = parse_marketplace_url(args.get("marketplaceUrl"), log) else {
            return log.last_error_code_or(-1);
        };
        let pat = match args.get("personalAccessToken").filter(|p| !p.is_empty()) {
            Some(p) => p.to_string(),
            None => match PublisherDb::new(&self.db_path).personal_access_token(publisher, log) {
                Some(p) => p,
                None => return log.last_error_code_or(-1),
            },
        };
        let client = match super::connect(self.factory.as_ref(), &pat, &url, log) {
            Ok(c) => c,
            Err(code) => return code,
        };
        if client.delete_extension(publisher, extension, log) == DeleteResult::Success {
            log.info(msg::INFO_EXTENSION_DELETED, &[&extension]);
        }
        log.last_error_code_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::fake::{args, Fake};
    use crate::logger::test_support::capture;

    #[test]
    fn deletes_using_stored_pat() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join(".vsce");
        let (mut log, out, _) = capture();
        PublisherDb::new(&db).add("me", "stored", false, &mut log);
        let fake = Fake::default();
        let cmd = DeleteExtensionCommand { db_path: db, factory: Box::new(fake.clone()) };
        let a = args(&[("publisherName", Some("me")), ("extensionName", Some("ext"))]);
        assert_eq!(cmd.execute(&a, &mut log), 0);
        assert_eq!(fake.0.borrow().pat.as_deref(), Some("stored"));
        assert_eq!(fake.0.borrow().calls, vec!["delete_extension me ext"]);
        assert_eq!(out.text(), "VSSDK: information VsixPub0037 : Removed 'ext' from the marketplace.\n");
    }

    #[test]
    fn missing_pat_and_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::default();
        let cmd = DeleteExtensionCommand { db_path: dir.path().join(".vsce"), factory: Box::new(fake.clone()) };
        let (mut log, _, _) = capture();
        let a = args(&[("publisherName", Some("me")), ("extensionName", Some("ext"))]);
        assert_eq!(cmd.execute(&a, &mut log), 7);
        fake.0.borrow_mut().delete_extension = Some(DeleteResult::NotFound);
        let a = args(&[
            ("publisherName", Some("me")),
            ("extensionName", Some("ext")),
            ("personalAccessToken", Some("p")),
        ]);
        assert_eq!(cmd.execute(&a, &mut log), 32);
    }
}
