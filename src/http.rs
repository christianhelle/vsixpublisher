//! Thin blocking HTTP layer over `ureq` (small binary, no async runtime).

use std::time::Duration;

use url::Url;

pub use ureq::http::StatusCode;

#[derive(Debug)]
pub enum HttpError {
    Timeout,
    Other(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::Timeout => f.write_str("The operation has timed out."),
            HttpError::Other(m) => f.write_str(m),
        }
    }
}

pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    pub fn new(timeout: Duration, user_agent: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .user_agent(user_agent)
            .build()
            .into();
        Self { agent }
    }

    /// Sends a request and returns the status and body text (empty when unreadable).
    pub fn send(
        &self,
        method: &str,
        url: &Url,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<(StatusCode, String), HttpError> {
        let mut b = ureq::http::Request::builder()
            .method(method)
            .uri(url.as_str());
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        let result = match body {
            Some(bytes) => b.body(bytes).map(|r| self.agent.run(r)),
            None => b.body(()).map(|r| self.agent.run(r)),
        }
        .map_err(|e| HttpError::Other(e.to_string()))?;
        let mut resp = result.map_err(|e| match e {
            ureq::Error::Timeout(_) => HttpError::Timeout,
            e => HttpError::Other(e.to_string()),
        })?;
        let status = resp.status();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        Ok((status, text))
    }
}

/// `Basic` authorization header value for `user:password`.
pub fn basic_auth(user: &str, password: &str) -> String {
    format!("Basic {}", base64(format!("{user}:{password}").as_bytes()))
}

fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encoding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(basic_auth("", "pat"), "Basic OnBhdA==");
    }

    #[test]
    fn sends_and_reads_non_success_without_error() {
        let mut s = mockito::Server::new();
        s.mock("POST", "/x")
            .match_header("h", "v")
            .match_body(vec![1u8, 2])
            .with_status(404)
            .with_body("missing")
            .create();
        let url = Url::parse(&format!("{}/x", s.url())).unwrap();
        let http = Http::new(Duration::from_secs(5), "ua");
        let (status, body) = http
            .send("POST", &url, &[("h", "v")], Some(&[1, 2]))
            .unwrap();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "missing");
    }
}
