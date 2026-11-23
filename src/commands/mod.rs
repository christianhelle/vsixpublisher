pub mod delete_extension;
pub mod delete_publisher;
pub mod login;
pub mod logout;

use crate::client::{ClientFactory, MarketplaceClient};
use crate::logger::Logger;
use reqwest::Url;

/// Creates a client, returning the exit code on failure.
pub(crate) fn connect(
    factory: &dyn ClientFactory,
    pat: &str,
    url: &Url,
    log: &mut Logger,
) -> Result<Box<dyn MarketplaceClient>, i32> {
    factory.create(pat, url, log).ok_or_else(|| log.last_error_code_or(-1))
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use crate::cli::{ArgValue, Args};
    use crate::client::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Records calls and returns canned results.
    #[derive(Default)]
    pub struct State {
        pub calls: Vec<String>,
        pub pat: Option<String>,
        pub publisher: Option<MarketplacePublisher>,
        pub get_extension: Option<GetExtension>,
        pub delete_extension: Option<DeleteResult>,
        pub delete_publisher_ok: bool,
        pub upload: Option<ExtensionResponse>,
        pub uploaded: Vec<u8>,
        pub auth_fails: bool,
    }

    #[derive(Clone, Default)]
    pub struct Fake(pub Rc<RefCell<State>>);

    impl ClientFactory for Fake {
        fn create(&self, pat: &str, _url: &Url, log: &mut Logger) -> Option<Box<dyn MarketplaceClient>> {
            self.0.borrow_mut().pat = Some(pat.to_string());
            if self.0.borrow().auth_fails {
                log.error(crate::messages::COULD_NOT_AUTHENTICATE, &[&"bad"]);
                return None;
            }
            Some(Box::new(self.clone()))
        }
    }

    impl MarketplaceClient for Fake {
        fn get_publisher(&self, p: &str, _: &mut Logger) -> Option<MarketplacePublisher> {
            self.0.borrow_mut().calls.push(format!("get_publisher {p}"));
            self.0.borrow().publisher.clone()
        }
        fn delete_publisher(&self, p: &str, _: &mut Logger) -> bool {
            self.0.borrow_mut().calls.push(format!("delete_publisher {p}"));
            self.0.borrow().delete_publisher_ok
        }
        fn get_extension(&self, p: &str, e: &str, _: &mut Logger) -> GetExtension {
            self.0.borrow_mut().calls.push(format!("get_extension {p} {e}"));
            self.0.borrow().get_extension.clone().unwrap_or(GetExtension::NotFound)
        }
        fn create_extension(&self, pkg: &[u8], p: &str, _: &mut Logger) -> Result<Option<ExtensionResponse>, String> {
            let mut s = self.0.borrow_mut();
            s.calls.push(format!("create_extension {p}"));
            s.uploaded = pkg.to_vec();
            Ok(s.upload.clone())
        }
        fn update_extension(&self, pkg: &[u8], p: &str, e: &str, _: &mut Logger) -> Result<Option<ExtensionResponse>, String> {
            let mut s = self.0.borrow_mut();
            s.calls.push(format!("update_extension {p} {e}"));
            s.uploaded = pkg.to_vec();
            Ok(s.upload.clone())
        }
        fn delete_extension(&self, p: &str, e: &str, log: &mut Logger) -> DeleteResult {
            self.0.borrow_mut().calls.push(format!("delete_extension {p} {e}"));
            let r = self.0.borrow().delete_extension.unwrap_or(DeleteResult::Success);
            if r == DeleteResult::NotFound {
                log.error(crate::messages::EXTENSION_NOT_FOUND, &[&e, &p]);
            }
            r
        }
    }

    pub fn args(pairs: &[(&'static str, Option<&str>)]) -> Args {
        let mut a = Args::default();
        for (k, v) in pairs {
            a.insert(k, v.map_or(ArgValue::Flag, |v| ArgValue::Value(v.to_string())));
        }
        a
    }
}
