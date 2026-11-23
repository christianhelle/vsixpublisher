//! Port of `MarketplaceHttpClient` / `MarketplaceClientFactory`.

use std::time::Duration;

use serde::Deserialize;
use url::Url;

use crate::http::{Http, HttpError, StatusCode, basic_auth};
use crate::logger::Logger;
use crate::messages as msg;

pub const DEFAULT_MARKETPLACE_URL: &str = "https://marketplace.visualstudio.com/";
const PUBLISHER_API_VERSION: &str = "4.1-preview.1";
const EXTENSION_API_VERSION: &str = "4.1-preview.2";
const EXTENSION_DOES_NOT_EXIST: &str = "ExtensionDoesNotExistException";
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

pub fn user_agent() -> String {
    format!("VsixPublisher/{}", env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketplacePublisher {
    pub publisher_id: Option<String>,
    pub publisher_name: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionResponse {
    pub extension_id: Option<String>,
    pub extension_name: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub message: Option<String>,
    pub type_key: Option<String>,
    #[serde(default)]
    pub error_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetExtension {
    Success(ExtensionResponse),
    NotFound,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteResult {
    Success,
    NotFound,
    Error,
}

pub trait MarketplaceClient {
    fn get_publisher(&self, publisher: &str, log: &mut Logger) -> Option<MarketplacePublisher>;
    fn delete_publisher(&self, publisher: &str, log: &mut Logger) -> bool;
    fn get_extension(&self, publisher: &str, extension: &str, log: &mut Logger) -> GetExtension;
    /// `Err` = transport failure (surfaced as a generic publishing error).
    fn create_extension(
        &self,
        package: &[u8],
        publisher: &str,
        log: &mut Logger,
    ) -> Result<Option<ExtensionResponse>, String>;
    fn update_extension(
        &self,
        package: &[u8],
        publisher: &str,
        extension: &str,
        log: &mut Logger,
    ) -> Result<Option<ExtensionResponse>, String>;
    fn delete_extension(&self, publisher: &str, extension: &str, log: &mut Logger) -> DeleteResult;
}

pub trait ClientFactory {
    /// Authenticates with `pat`; logs and returns `None` on failure.
    fn create(&self, pat: &str, url: &Url, log: &mut Logger) -> Option<Box<dyn MarketplaceClient>>;
}

pub struct HttpClientFactory;

impl ClientFactory for HttpClientFactory {
    fn create(&self, pat: &str, url: &Url, log: &mut Logger) -> Option<Box<dyn MarketplaceClient>> {
        let client = HttpMarketplaceClient::new(url.clone(), pat.to_string());
        client
            .authenticate(log)
            .then(|| Box::new(client) as Box<dyn MarketplaceClient>)
    }
}

pub struct HttpMarketplaceClient {
    base: Url,
    pat: String,
    http: Http,
}

/// A request to the marketplace API.
struct Req {
    method: &'static str,
    url: Url,
    api_version: &'static str,
}

/// Mirrors `VssHttpClientBase` error handling: wrapped-exception JSON or a status-based message.
fn service_error(url: &Url, status: StatusCode, body: &str) -> ErrorResponse {
    if let Ok(e) = serde_json::from_str::<ErrorResponse>(body)
        && e.message.is_some()
    {
        return e;
    }
    let message = if status == StatusCode::UNAUTHORIZED {
        format!(
            "VS30063: You are not authorized to access {}.",
            url.origin().ascii_serialization()
        )
    } else {
        let reason = status.canonical_reason().unwrap_or("");
        format!("{} ({}).", reason, status.as_u16())
            .trim_start()
            .to_string()
    };
    ErrorResponse {
        message: Some(message),
        type_key: None,
        error_code: 0,
    }
}

impl HttpMarketplaceClient {
    pub fn new(base: Url, pat: String) -> Self {
        let http = Http::new(REQUEST_TIMEOUT, &user_agent());
        Self { base, pat, http }
    }

    fn url(&self, relative: &str) -> Url {
        self.base.join(relative).expect("valid relative url")
    }

    fn publisher_url(&self, p: &str) -> Url {
        self.url(&format!("_apis/gallery/publishers/{p}/"))
    }

    fn extension_url(&self, p: &str, e: &str) -> Url {
        self.url(&format!("_apis/gallery/publishers/{p}/extensions/{e}/"))
    }

    fn request(&self, method: &'static str, url: Url, api_version: &'static str) -> Req {
        Req {
            method,
            url,
            api_version,
        }
    }

    fn send(&self, req: Req) -> Result<(StatusCode, String), HttpError> {
        self.send_with(&self.http, req, None)
    }

    fn send_with(
        &self,
        http: &Http,
        req: Req,
        body: Option<&[u8]>,
    ) -> Result<(StatusCode, String), HttpError> {
        let auth = basic_auth("", &self.pat);
        let accept = format!("application/json; api-version={}", req.api_version);
        let mut headers = vec![
            ("Authorization", auth.as_str()),
            ("Accept", accept.as_str()),
            ("X-TFS-FedAuthRedirect", "Suppress"),
        ];
        if body.is_some() {
            headers.push(("Content-Type", "application/octet-stream"));
        }
        http.send(req.method, &req.url, &headers, body)
    }

    /// Equivalent of `VssConnection.ConnectAsync`.
    pub fn authenticate(&self, log: &mut Logger) -> bool {
        if !is_secure_or_loopback(&self.base) {
            log.error(
                msg::COULD_NOT_AUTHENTICATE,
                &[&"Basic authentication requires a secure connection to the server."],
            );
            return false;
        }
        let url =
            self.url("_apis/connectionData?connectOptions=1&lastChangeId=-1&lastChangeId64=-1");
        let rb = self.request("GET", url, "1.0");
        match self.send_with(&Http::new(AUTH_TIMEOUT, &user_agent()), rb, None) {
            Ok((s, _)) if s.is_success() => true,
            Ok((s, body)) => {
                let e = service_error(&self.base, s, &body);
                log.error(
                    msg::COULD_NOT_AUTHENTICATE,
                    &[&e.message.unwrap_or_default()],
                );
                false
            }
            Err(HttpError::Timeout) => {
                log.error(msg::AUTHENTICATION_TIMEOUT, &[]);
                false
            }
            Err(e) => {
                log.error(msg::COULD_NOT_AUTHENTICATE, &[&e]);
                false
            }
        }
    }

    fn upload(
        &self,
        rb: Req,
        package: &[u8],
        log: &mut Logger,
    ) -> Result<Option<ExtensionResponse>, String> {
        let (status, body) = self
            .send_with(&self.http, rb, Some(package))
            .map_err(|e| e.to_string())?;
        if status.is_success() {
            return Ok(serde_json::from_str(&body).ok());
        }
        let e = service_error(&self.base, status, &body);
        let m = if e.error_code == 1 {
            msg::WARNINGS_FROM_MARKETPLACE
        } else {
            msg::ERROR_FROM_MARKETPLACE
        };
        log.error(m, &[&e.message.unwrap_or_default()]);
        Ok(None)
    }
}

impl MarketplaceClient for HttpMarketplaceClient {
    fn get_publisher(&self, publisher: &str, log: &mut Logger) -> Option<MarketplacePublisher> {
        let rb = self.request("GET", self.publisher_url(publisher), PUBLISHER_API_VERSION);
        match self.send(rb) {
            Ok((s, body)) if s.is_success() => match serde_json::from_str(&body) {
                Ok(p) => Some(p),
                Err(e) => {
                    log.error(msg::ERROR_FROM_MARKETPLACE, &[&e]);
                    None
                }
            },
            Ok((StatusCode::NOT_FOUND, _)) => None,
            Ok((_, body)) => {
                log.error(msg::ERROR_FROM_MARKETPLACE, &[&body]);
                None
            }
            Err(e) => {
                log.error(msg::ERROR_FROM_MARKETPLACE, &[&e]);
                None
            }
        }
    }

    fn delete_publisher(&self, publisher: &str, log: &mut Logger) -> bool {
        let rb = self.request(
            "DELETE",
            self.publisher_url(publisher),
            PUBLISHER_API_VERSION,
        );
        match self.send(rb) {
            Ok((s, _)) if s.is_success() => {}
            Ok((s, body)) => {
                let e = service_error(&self.base, s, &body);
                log.error(
                    msg::ERROR_FROM_MARKETPLACE,
                    &[&e.message.unwrap_or_default()],
                );
            }
            Err(e) => {
                log.error(msg::ERROR_FROM_MARKETPLACE, &[&e]);
            }
        }
        log.last_error().is_none()
    }

    fn get_extension(&self, publisher: &str, extension: &str, log: &mut Logger) -> GetExtension {
        let rb = self.request(
            "GET",
            self.extension_url(publisher, extension),
            EXTENSION_API_VERSION,
        );
        match self.send(rb) {
            Ok((s, body)) if s.is_success() => match serde_json::from_str(&body) {
                Ok(r) => GetExtension::Success(r),
                Err(e) => {
                    log.error(msg::ERROR_FROM_MARKETPLACE, &[&e]);
                    GetExtension::Error
                }
            },
            Ok((s, body)) => {
                let e = service_error(&self.base, s, &body);
                if s == StatusCode::NOT_FOUND
                    && e.type_key.as_deref() == Some(EXTENSION_DOES_NOT_EXIST)
                {
                    return GetExtension::NotFound;
                }
                log.error(
                    msg::ERROR_FROM_MARKETPLACE,
                    &[&e.message.unwrap_or_default()],
                );
                GetExtension::Error
            }
            Err(e) => {
                log.error(msg::ERROR_FROM_MARKETPLACE, &[&e]);
                GetExtension::Error
            }
        }
    }

    fn create_extension(
        &self,
        package: &[u8],
        publisher: &str,
        log: &mut Logger,
    ) -> Result<Option<ExtensionResponse>, String> {
        let url = self.url(&format!("_apis/gallery/publishers/{publisher}/extensions/"));
        self.upload(
            self.request("POST", url, EXTENSION_API_VERSION),
            package,
            log,
        )
    }

    fn update_extension(
        &self,
        package: &[u8],
        publisher: &str,
        extension: &str,
        log: &mut Logger,
    ) -> Result<Option<ExtensionResponse>, String> {
        let url = self.extension_url(publisher, extension);
        self.upload(
            self.request("PUT", url, EXTENSION_API_VERSION),
            package,
            log,
        )
    }

    fn delete_extension(&self, publisher: &str, extension: &str, log: &mut Logger) -> DeleteResult {
        let rb = self.request(
            "DELETE",
            self.extension_url(publisher, extension),
            EXTENSION_API_VERSION,
        );
        match self.send(rb) {
            Ok((s, _)) if s.is_success() => DeleteResult::Success,
            Ok((s, body)) => {
                let e = service_error(&self.base, s, &body);
                if s == StatusCode::NOT_FOUND
                    && e.type_key.as_deref() == Some(EXTENSION_DOES_NOT_EXIST)
                {
                    log.error(msg::EXTENSION_NOT_FOUND, &[&extension, &publisher]);
                    return DeleteResult::NotFound;
                }
                log.error(
                    msg::ERROR_FROM_MARKETPLACE,
                    &[&e.message.unwrap_or_default()],
                );
                DeleteResult::Error
            }
            Err(e) => {
                log.error(msg::ERROR_FROM_MARKETPLACE, &[&e]);
                DeleteResult::Error
            }
        }
    }
}

/// `VssBasicCredential` refuses plain HTTP; loopback is allowed so the tool can be tested locally.
pub fn is_secure_or_loopback(url: &Url) -> bool {
    if url.scheme() == "https" {
        return true;
    }
    match url.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// Parses a `-marketplaceUrl` value; logs VsixPub0034 on failure.
pub fn parse_marketplace_url(value: Option<&str>, log: &mut Logger) -> Option<Url> {
    let text = value.unwrap_or(DEFAULT_MARKETPLACE_URL);
    match Url::parse(text) {
        Ok(u) => Some(u),
        Err(_) => {
            log.error(msg::INVALID_URI, &[&text, &"marketplaceUrl"]);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::test_support::capture;
    use mockito::{Matcher, Server};

    const AUTH: &str = "Basic OnBhdA=="; // ":pat"

    fn client(server: &Server) -> HttpMarketplaceClient {
        HttpMarketplaceClient::new(
            Url::parse(&format!("{}/", server.url())).unwrap(),
            "pat".into(),
        )
    }

    #[test]
    fn authenticate_ok_and_unauthorized() {
        let mut s = Server::new();
        let ok = s
            .mock("GET", "/_apis/connectionData")
            .match_query(Matcher::Any)
            .match_header("authorization", AUTH)
            .match_header("x-tfs-fedauthredirect", "Suppress")
            .with_status(200)
            .with_body("{}")
            .create();
        let (mut log, _, _) = capture();
        assert!(client(&s).authenticate(&mut log));
        ok.assert();
        ok.remove();
        s.mock("GET", "/_apis/connectionData")
            .match_query(Matcher::Any)
            .with_status(401)
            .with_body(r#"{"message":"TF400813: nope","typeKey":"UnauthorizedRequestException","errorCode":0}"#)
            .create();
        let (mut log, _, err) = capture();
        assert!(!client(&s).authenticate(&mut log));
        assert_eq!(
            err.text(),
            "VSSDK: error VsixPub0031 : An error occurred while attempting to authenticate: TF400813: nope\n"
        );
    }

    #[test]
    fn get_publisher_found_and_missing() {
        let mut s = Server::new();
        s.mock("GET", "/_apis/gallery/publishers/me/")
            .match_header("accept", "application/json; api-version=4.1-preview.1")
            .with_body(r#"{"publisherName":"me","displayName":"Me"}"#)
            .create();
        s.mock("GET", "/_apis/gallery/publishers/ghost/")
            .with_status(404)
            .create();
        let (mut log, _, _) = capture();
        let c = client(&s);
        assert_eq!(
            c.get_publisher("me", &mut log)
                .unwrap()
                .display_name
                .as_deref(),
            Some("Me")
        );
        assert!(c.get_publisher("ghost", &mut log).is_none());
        assert!(log.last_error().is_none());
    }

    #[test]
    fn get_extension_variants() {
        let mut s = Server::new();
        s.mock("GET", "/_apis/gallery/publishers/me/extensions/ok/")
            .match_header("accept", "application/json; api-version=4.1-preview.2")
            .with_body(r#"{"displayName":"Ok Ext"}"#)
            .create();
        s.mock("GET", "/_apis/gallery/publishers/me/extensions/new/")
            .with_status(404)
            .with_body(r#"{"message":"x","typeKey":"ExtensionDoesNotExistException"}"#)
            .create();
        s.mock("GET", "/_apis/gallery/publishers/me/extensions/bad/")
            .with_status(500)
            .with_body(r#"{"message":"boom","typeKey":"Other"}"#)
            .create();
        let (mut log, _, err) = capture();
        let c = client(&s);
        assert!(
            matches!(c.get_extension("me", "ok", &mut log), GetExtension::Success(r) if r.display_name.as_deref() == Some("Ok Ext"))
        );
        assert_eq!(
            c.get_extension("me", "new", &mut log),
            GetExtension::NotFound
        );
        assert_eq!(c.get_extension("me", "bad", &mut log), GetExtension::Error);
        assert!(err.text().contains(
            "VsixPub0029 : An error occurred while communicating with the marketplace: boom"
        ));
    }

    #[test]
    fn upload_create_update_and_warnings() {
        let mut s = Server::new();
        let create = s
            .mock("POST", "/_apis/gallery/publishers/me/extensions/")
            .match_header("content-type", "application/octet-stream")
            .match_body(vec![1u8, 2, 3])
            .with_body(r#"{"displayName":"New"}"#)
            .create();
        s.mock("PUT", "/_apis/gallery/publishers/me/extensions/x/")
            .with_status(400)
            .with_body(r#"{"message":"VSIXValidatorWarning01","errorCode":1}"#)
            .create();
        let (mut log, _, err) = capture();
        let c = client(&s);
        let r = c
            .create_extension(&[1, 2, 3], "me", &mut log)
            .unwrap()
            .unwrap();
        assert_eq!(r.display_name.as_deref(), Some("New"));
        create.assert();
        assert!(
            c.update_extension(&[1], "me", "x", &mut log)
                .unwrap()
                .is_none()
        );
        assert_eq!(log.last_error_code_or(0), 43);
        assert!(err.text().contains(
            "preventing it from being uploaded to the Marketplace: VSIXValidatorWarning01"
        ));
    }

    #[test]
    fn delete_extension_not_found() {
        let mut s = Server::new();
        s.mock("DELETE", "/_apis/gallery/publishers/me/extensions/x/")
            .with_status(404)
            .with_body(r#"{"message":"m","typeKey":"ExtensionDoesNotExistException"}"#)
            .create();
        s.mock("DELETE", "/_apis/gallery/publishers/me/extensions/y/")
            .with_status(204)
            .create();
        let (mut log, _, _) = capture();
        let c = client(&s);
        assert_eq!(
            c.delete_extension("me", "y", &mut log),
            DeleteResult::Success
        );
        assert_eq!(
            c.delete_extension("me", "x", &mut log),
            DeleteResult::NotFound
        );
        assert_eq!(log.last_error_code_or(0), 32);
    }

    #[test]
    fn delete_publisher_reports_errors() {
        let mut s = Server::new();
        s.mock("DELETE", "/_apis/gallery/publishers/me/")
            .with_status(200)
            .create();
        s.mock("DELETE", "/_apis/gallery/publishers/no/")
            .with_status(403)
            .with_body(r#"{"message":"denied"}"#)
            .create();
        let (mut log, _, _) = capture();
        let c = client(&s);
        assert!(c.delete_publisher("me", &mut log));
        assert!(!c.delete_publisher("no", &mut log));
    }

    #[test]
    fn plain_http_is_refused_except_loopback() {
        let (mut log, _, err) = capture();
        let c =
            HttpMarketplaceClient::new(Url::parse("http://example.com/").unwrap(), "pat".into());
        assert!(!c.authenticate(&mut log));
        assert_eq!(
            err.text(),
            "VSSDK: error VsixPub0031 : An error occurred while attempting to authenticate: Basic authentication requires a secure connection to the server.\n"
        );
        assert!(is_secure_or_loopback(
            &Url::parse("http://127.0.0.1:1/").unwrap()
        ));
        assert!(is_secure_or_loopback(
            &Url::parse("http://localhost/").unwrap()
        ));
        assert!(is_secure_or_loopback(
            &Url::parse("https://example.com/").unwrap()
        ));
    }

    #[test]
    fn parse_url() {
        let (mut log, _, _) = capture();
        assert_eq!(
            parse_marketplace_url(None, &mut log).unwrap().as_str(),
            DEFAULT_MARKETPLACE_URL
        );
        assert!(parse_marketplace_url(Some("not a url"), &mut log).is_none());
        assert_eq!(log.last_error_code_or(0), 34);
    }
}
