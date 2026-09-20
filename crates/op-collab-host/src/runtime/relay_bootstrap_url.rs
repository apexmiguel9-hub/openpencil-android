use reqwest::Url;

use super::MAX_URL_BYTES;

pub(super) fn parse(value: &str, path: &str) -> Option<Url> {
    if value.is_empty() || value.len() > MAX_URL_BYTES || value.trim() != value || !value.is_ascii()
    {
        return None;
    }
    let endpoint = Url::parse(value).ok()?;
    let host = endpoint.host_str()?;
    if host.is_empty()
        || !host.is_ascii()
        || host != host.to_ascii_lowercase()
        || host.contains('%')
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.path() != path
        || endpoint.port().is_some_and(|port| port == 0 || port == 443)
        || endpoint.as_str() != value
    {
        return None;
    }
    Some(endpoint)
}
