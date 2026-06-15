use anyhow::{Context, Result};
use reqwest::header::{HeaderName, HeaderValue};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequestHeader {
    name: HeaderName,
    value: HeaderValue,
    display_value: String,
}

impl HttpRequestHeader {
    pub fn new(name: impl AsRef<str>, value: impl Into<String>) -> Result<Self> {
        let name = HeaderName::from_bytes(name.as_ref().trim().as_bytes())
            .context("AI HTTP header name is invalid")?;
        let display_value = value.into();
        let value =
            HeaderValue::from_str(&display_value).context("AI HTTP header value is invalid")?;
        Ok(Self {
            name,
            value,
            display_value,
        })
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn value(&self) -> &str {
        &self.display_value
    }
}

pub fn post_json<T, R>(
    client: &reqwest::blocking::Client,
    url: &str,
    body: &T,
    headers: &[HttpRequestHeader],
) -> Result<R>
where
    T: Serialize + ?Sized,
    R: DeserializeOwned,
{
    let mut request = client.post(url).json(body);
    for header in headers {
        request = request.header(header.name.clone(), header.value.clone());
    }

    let response = request.send().context("AI HTTP request failed")?;
    let status = response.status();
    let text = response.text().context("AI HTTP response read failed")?;
    anyhow::ensure!(
        status.is_success(),
        "AI HTTP request returned status {}",
        status.as_u16()
    );
    serde_json::from_str(&text).context("AI HTTP response JSON parse failed")
}

pub fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}
