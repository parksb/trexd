use http::{header::HeaderName, request::Parts, uri::Authority, HeaderMap};
use ipnet::IpNet;
use serde::{Deserialize, Deserializer};
use std::net::IpAddr;

#[derive(Clone, Debug, Deserialize)]
pub struct ClientIp {
    #[serde(deserialize_with = "deserialize_header")]
    pub header: HeaderName,
    pub trusted_proxies: Vec<IpNet>,
}

fn deserialize_header<'de, D: Deserializer<'de>>(d: D) -> Result<HeaderName, D::Error> {
    String::deserialize(d)?
        .parse()
        .map_err(serde::de::Error::custom)
}

impl ClientIp {
    pub fn resolve(&self, peer: IpAddr, headers: &HeaderMap) -> Result<IpAddr, ()> {
        if !self.trusted_proxies.iter().any(|net| net.contains(&peer)) {
            return Ok(peer.to_canonical());
        }

        let mut values = headers.get_all(&self.header).iter();
        let value = values.next().ok_or(())?;
        if values.next().is_some() {
            return Err(());
        }

        let ip: IpAddr = value.to_str().map_err(|_| ())?.parse().map_err(|_| ())?;
        Ok(ip.to_canonical())
    }
}

pub fn normalize_host(value: &str) -> Option<String> {
    if value.contains('@') {
        return None;
    }

    let authority: Authority = value.parse().ok()?;
    let host = authority.host().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || host.len() > 253 {
        return None;
    }

    let suffix = &value[authority.host().len()..];
    if !suffix.is_empty() && suffix.strip_prefix(':')?.parse::<u16>().is_err() {
        return None;
    }

    Some(host)
}

// Validate transport-level bounds before opening an upstream connection.
pub fn validate_request(request: &Parts) -> Result<(), u16> {
    if request.uri.to_string().len() > 8192 {
        return Err(414);
    }

    let size: usize = request
        .headers
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len() + 4)
        .sum();

    if size > 16_384 || request.headers.len() > 128 {
        return Err(431);
    }

    Ok(())
}

// Use the Host header (HTTP/1) or URI authority (HTTP/2), rejecting ambiguity.
pub fn request_host(request: &Parts) -> Option<String> {
    let mut values = request.headers.get_all(http::header::HOST).iter();
    let host = match values.next() {
        Some(value) => normalize_host(value.to_str().ok()?),
        None => request
            .uri
            .authority()
            .and_then(|a| normalize_host(a.as_str())),
    }?;

    if values.next().is_some() {
        return None;
    }

    if let Some(authority) = request.uri.authority() {
        if normalize_host(authority.as_str()).as_deref() != Some(&host) {
            return None;
        }
    }

    Some(host)
}
