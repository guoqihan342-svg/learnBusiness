use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn post_json<T, R>(
    client: &reqwest::blocking::Client,
    url: &str,
    body: &T,
    bearer_token: Option<&str>,
) -> Result<R>
where
    T: Serialize + ?Sized,
    R: DeserializeOwned,
{
    let mut request = client.post(url).json(body);
    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
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
