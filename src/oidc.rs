//! Port of `GitHubActionsOidcTokenProvider` (trusted publishing).

use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::client::{AUTH_TIMEOUT, user_agent};
use crate::logger::Logger;
use crate::messages as msg;

pub const AUDIENCE: &str = "marketplace.visualstudio.com";
pub const REQUEST_TOKEN_VAR: &str = "ACTIONS_ID_TOKEN_REQUEST_TOKEN";
pub const REQUEST_URL_VAR: &str = "ACTIONS_ID_TOKEN_REQUEST_URL";
const TOKEN_EXCHANGE_PATH: &str = "_apis/gallery/token?api-version=7.2-preview.1";
const CI_PROVIDER: &str = "github-actions";

pub trait TokenProvider {
    /// Returns a marketplace session token, logging errors on failure.
    fn session_token(&self, publisher: &str, marketplace: &Url, log: &mut Logger)
    -> Option<String>;
}

/// Environment variable lookup (injectable for tests).
pub type EnvLookup = Box<dyn Fn(&str) -> Option<String>>;

pub struct GitHubActionsTokenProvider {
    pub env: EnvLookup,
}

impl Default for GitHubActionsTokenProvider {
    fn default() -> Self {
        Self {
            env: Box::new(|k| std::env::var(k).ok()),
        }
    }
}

#[derive(Deserialize)]
struct OidcResponse {
    value: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    credential: Option<String>,
}

/// Appends `audience=marketplace.visualstudio.com` to the request URL's query.
pub fn token_request_url(raw: &str) -> Option<Url> {
    let mut u = Url::parse(raw).ok()?;
    let aud = format!("audience={AUDIENCE}");
    let q = match u.query().filter(|q| !q.is_empty()) {
        Some(q) => format!("{q}&{aud}"),
        None => aud,
    };
    u.set_query(Some(&q));
    Some(u)
}

enum Fail {
    Timeout,
    Other(String),
}

fn send(rb: reqwest::blocking::RequestBuilder) -> Result<(bool, String), Fail> {
    let r = rb.send().map_err(|e| {
        if e.is_timeout() {
            Fail::Timeout
        } else {
            Fail::Other(e.to_string())
        }
    })?;
    let ok = r.status().is_success();
    Ok((ok, r.text().unwrap_or_default()))
}

impl TokenProvider for GitHubActionsTokenProvider {
    fn session_token(
        &self,
        publisher: &str,
        marketplace: &Url,
        log: &mut Logger,
    ) -> Option<String> {
        let token = (self.env)(REQUEST_TOKEN_VAR).filter(|s| !s.is_empty());
        let url = (self.env)(REQUEST_URL_VAR).filter(|s| !s.is_empty());
        let (Some(token), Some(url)) = (token, url) else {
            log.error(
                msg::OIDC_PROVIDER_UNAVAILABLE,
                &[&REQUEST_URL_VAR, &REQUEST_TOKEN_VAR],
            );
            return None;
        };
        let Some(request_url) = token_request_url(&url) else {
            log.error(msg::INVALID_URI, &[&url, &REQUEST_URL_VAR]);
            return None;
        };
        let http = Client::builder()
            .timeout(AUTH_TIMEOUT)
            .user_agent(user_agent())
            .build()
            .ok()?;

        let request = http
            .get(request_url)
            .bearer_auth(&token)
            .header("Accept", "application/json");
        let provider_token = match send(request) {
            Err(Fail::Timeout) => {
                log.error(msg::AUTHENTICATION_TIMEOUT, &[]);
                return None;
            }
            Err(Fail::Other(e)) => {
                log.error(msg::OIDC_PROVIDER_TOKEN_REQUEST_FAILED, &[&e]);
                return None;
            }
            Ok((false, body)) => {
                log.error(msg::OIDC_PROVIDER_TOKEN_REQUEST_FAILED, &[&body]);
                return None;
            }
            Ok((true, body)) => match serde_json::from_str::<OidcResponse>(&body) {
                Ok(OidcResponse { value: Some(v) }) if !v.is_empty() => v,
                Ok(_) => {
                    log.error(
                        msg::OIDC_PROVIDER_TOKEN_REQUEST_FAILED,
                        &[&msg::OIDC_TOKEN_RESPONSE_MISSING_TOKEN],
                    );
                    return None;
                }
                Err(e) => {
                    log.error(msg::OIDC_PROVIDER_TOKEN_REQUEST_FAILED, &[&e]);
                    return None;
                }
            },
        };

        // Same contract as vsce; the original VsixPublisher's Bearer exchange is rejected.
        let exchange = marketplace.join(TOKEN_EXCHANGE_PATH).ok()?;
        let body = serde_json::json!({ "publisherName": publisher }).to_string();
        let rb = http
            .post(exchange)
            .header("Authorization", format!("FederatedToken {provider_token}"))
            .header("Accept", "application/json")
            .header("X-Market-CI-Provider", CI_PROVIDER)
            .header("Content-Type", "application/json; charset=utf-8")
            .body(body);
        match send(rb) {
            Err(Fail::Timeout) => {
                log.error(msg::AUTHENTICATION_TIMEOUT, &[]);
                None
            }
            Err(Fail::Other(e)) => {
                log.error(msg::OIDC_SESSION_TOKEN_EXCHANGE_FAILED, &[&e]);
                None
            }
            Ok((false, body)) => {
                log.error(msg::OIDC_SESSION_TOKEN_EXCHANGE_FAILED, &[&body]);
                None
            }
            Ok((true, body)) => match serde_json::from_str::<TokenResponse>(&body) {
                Ok(TokenResponse {
                    credential: Some(c),
                }) if !c.is_empty() => {
                    if (self.env)("GITHUB_ACTIONS").as_deref() == Some("true") {
                        // Keep the short-lived credential out of the workflow log.
                        log.message(&format!("::add-mask::{c}"));
                    }
                    Some(c)
                }
                Ok(_) => {
                    log.error(
                        msg::OIDC_SESSION_TOKEN_EXCHANGE_FAILED,
                        &[&msg::OIDC_TOKEN_RESPONSE_MISSING_TOKEN],
                    );
                    None
                }
                Err(e) => {
                    log.error(msg::OIDC_SESSION_TOKEN_EXCHANGE_FAILED, &[&e]);
                    None
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::test_support::capture;
    use mockito::{Matcher, Server};
    use std::collections::HashMap;

    fn provider(vars: &[(&str, String)]) -> GitHubActionsTokenProvider {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        GitHubActionsTokenProvider {
            env: Box::new(move |k| map.get(k).cloned()),
        }
    }

    #[test]
    fn appends_audience() {
        assert_eq!(
            token_request_url("https://x/token?api-version=2.0")
                .unwrap()
                .as_str(),
            "https://x/token?api-version=2.0&audience=marketplace.visualstudio.com"
        );
        assert_eq!(
            token_request_url("https://x/t").unwrap().query(),
            Some("audience=marketplace.visualstudio.com")
        );
        assert!(token_request_url("nope").is_none());
    }

    #[test]
    fn missing_env_vars() {
        let (mut log, _, err) = capture();
        let url = Url::parse("https://m/").unwrap();
        assert!(provider(&[]).session_token("me", &url, &mut log).is_none());
        assert_eq!(log.last_error_code_or(0), 45);
        assert!(
            err.text()
                .contains("'ACTIONS_ID_TOKEN_REQUEST_URL' and 'ACTIONS_ID_TOKEN_REQUEST_TOKEN'")
        );
    }

    #[test]
    fn exchanges_tokens() {
        let mut s = Server::new();
        s.mock("GET", "/oidc")
            .match_query(Matcher::UrlEncoded("audience".into(), AUDIENCE.into()))
            .match_header("authorization", "Bearer req-token")
            .match_header("accept", "application/json")
            .with_body(r#"{"value":"gh-jwt"}"#)
            .create();
        let exchange = s
            .mock("POST", "/_apis/gallery/token")
            .match_query(Matcher::UrlEncoded(
                "api-version".into(),
                "7.2-preview.1".into(),
            ))
            .match_header("authorization", "FederatedToken gh-jwt")
            .match_header("accept", "application/json")
            .match_header("x-market-ci-provider", "github-actions")
            .match_header("content-type", Matcher::Regex("^application/json".into()))
            .match_body(Matcher::Json(serde_json::json!({"publisherName":"me"})))
            .with_body(r#"{"credential":"session"}"#)
            .create();
        let p = provider(&[
            (REQUEST_TOKEN_VAR, "req-token".into()),
            (REQUEST_URL_VAR, format!("{}/oidc", s.url())),
            ("GITHUB_ACTIONS", "true".into()),
        ]);
        let (mut log, out, _) = capture();
        let url = Url::parse(&format!("{}/", s.url())).unwrap();
        assert_eq!(
            p.session_token("me", &url, &mut log).as_deref(),
            Some("session")
        );
        exchange.assert();
        assert_eq!(
            out.text(),
            "::add-mask::session
"
        );
    }

    #[test]
    fn provider_and_exchange_failures() {
        let mut s = Server::new();
        s.mock("GET", "/oidc")
            .match_query(Matcher::Any)
            .with_body("{}")
            .create();
        let p = provider(&[
            (REQUEST_TOKEN_VAR, "t".into()),
            (REQUEST_URL_VAR, format!("{}/oidc", s.url())),
        ]);
        let (mut log, _, err) = capture();
        let url = Url::parse(&format!("{}/", s.url())).unwrap();
        assert!(p.session_token("me", &url, &mut log).is_none());
        assert_eq!(log.last_error_code_or(0), 46);
        assert!(
            err.text()
                .contains("The token response did not contain a token.")
        );

        let mut s = Server::new();
        s.mock("GET", "/oidc")
            .match_query(Matcher::Any)
            .with_body(r#"{"value":"j"}"#)
            .create();
        s.mock("POST", "/_apis/gallery/token")
            .with_status(403)
            .with_body("forbidden")
            .create();
        let p = provider(&[
            (REQUEST_TOKEN_VAR, "t".into()),
            (REQUEST_URL_VAR, format!("{}/oidc", s.url())),
        ]);
        let url = Url::parse(&format!("{}/", s.url())).unwrap();
        assert!(p.session_token("me", &url, &mut log).is_none());
        assert_eq!(log.last_error_code_or(0), 47);
    }
}
