//! Geolocation of client IP addresses, for the event log.
//!
//! Lookups go to external services (via the public-ip-address crate), so
//! results are cached in memory per address: successful ones for a day and
//! failures for an hour. This keeps repeated requests from one client fast
//! and stays within the free services' quotas. Private, loopback and other
//! non-routable addresses are never looked up.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use public_ip_address::response::LookupResponse;

const SUCCESS_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const FAILURE_TTL: Duration = Duration::from_secs(60 * 60);
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ENTRIES: usize = 10_000;

type CacheEntry = (Instant, Option<LookupResponse>);

#[derive(Clone, Default)]
pub struct GeoIp {
    cache: Arc<Mutex<HashMap<IpAddr, CacheEntry>>>,
}

impl GeoIp {
    pub fn new() -> Self {
        Self::default()
    }

    /// The location of a public address, or None if it is not public or the
    /// lookup failed
    pub async fn lookup(&self, ip: IpAddr) -> Option<LookupResponse> {
        let ip = ip.to_canonical();
        if !is_public(&ip) {
            return None;
        }
        if let Some(cached) = self.cached(&ip) {
            return cached;
        }

        let result = match tokio::time::timeout(LOOKUP_TIMEOUT, public_ip_address::perform_lookup(Some(ip))).await {
            Ok(Ok(response)) => Some(response),
            Ok(Err(e)) => {
                warn!("Failed to look up location of {ip}: {e}");
                None
            }
            Err(_) => {
                warn!("Timed out looking up location of {ip}");
                None
            }
        };
        self.store(ip, result.clone());
        result
    }

    fn cached(&self, ip: &IpAddr) -> Option<Option<LookupResponse>> {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.get(ip)
            .filter(|entry| !is_expired(entry))
            .map(|(_, response)| response.clone())
    }

    fn store(&self, ip: IpAddr, response: Option<LookupResponse>) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() >= MAX_ENTRIES {
            cache.retain(|_, entry| !is_expired(entry));
            if cache.len() >= MAX_ENTRIES {
                cache.clear();
            }
        }
        cache.insert(ip, (Instant::now(), response));
    }
}

fn is_expired((at, response): &CacheEntry) -> bool {
    at.elapsed() >= if response.is_some() { SUCCESS_TTL } else { FAILURE_TTL }
}

/// Whether an address is routable on the public internet, and so worth
/// looking up
pub fn is_public(ip: &IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            let shared = a == 100 && (64..128).contains(&b); // 100.64.0.0/10, carrier-grade NAT
            !(v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()
                || v4.is_broadcast() || v4.is_documentation() || v4.is_multicast() || shared)
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            let unique_local = (first & 0xfe00) == 0xfc00; // fc00::/7
            let link_local = (first & 0xffc0) == 0xfe80; // fe80::/10
            !(v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() || unique_local || link_local)
        }
    }
}

/// A one-line description of a location, e.g.
/// "Leeds, England, United Kingdom (GB); AS2856 British Telecommunications PLC; proxy"
pub fn summarize(response: &LookupResponse) -> String {
    let place: Vec<&str> = [&response.city, &response.region, &response.country]
        .into_iter()
        .filter_map(|part| part.as_deref())
        .filter(|part| !part.is_empty())
        .collect();
    let mut summary = if place.is_empty() { "unknown location".to_string() } else { place.join(", ") };
    if let Some(code) = response.country_code.as_deref().filter(|c| !c.is_empty()) {
        summary.push_str(&format!(" ({code})"));
    }

    let network: Vec<&str> = [&response.asn, &response.asn_org]
        .into_iter()
        .filter_map(|part| part.as_deref())
        .filter(|part| !part.is_empty())
        .collect();
    if !network.is_empty() {
        summary.push_str("; ");
        summary.push_str(&network.join(" "));
    }
    if let Some(hostname) = response.hostname.as_deref().filter(|h| !h.is_empty()) {
        summary.push_str(&format!("; host {hostname}"));
    }
    if response.is_proxy == Some(true) {
        summary.push_str("; proxy");
    }
    summary
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use public_ip_address::lookup::LookupProvider;
    use public_ip_address::response::LookupResponse;

    use super::{is_public, summarize, GeoIp};

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn test_is_public() {
        for public in ["8.8.8.8", "81.2.69.160", "2a00:1450:4009:81f::200e", "::ffff:81.2.69.160"] {
            assert!(is_public(&ip(public)), "{public} is public");
        }
        for private in ["127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.1.1", "100.64.0.1", "169.254.1.1",
                        "0.0.0.0", "::1", "fd00::1", "fe80::1", "::ffff:127.0.0.1"] {
            assert!(!is_public(&ip(private)), "{private} is not public");
        }
    }

    #[test]
    fn test_summarize() {
        let mut response = LookupResponse::new(ip("81.2.69.160"), LookupProvider::IpInfo);
        assert_eq!("unknown location", summarize(&response));

        response.city = Some("Leeds".to_string());
        response.region = Some("England".to_string());
        response.country = Some("United Kingdom".to_string());
        response.country_code = Some("GB".to_string());
        response.asn = Some("AS2856".to_string());
        response.asn_org = Some("British Telecommunications PLC".to_string());
        response.is_proxy = Some(true);
        assert_eq!("Leeds, England, United Kingdom (GB); AS2856 British Telecommunications PLC; proxy", summarize(&response));
    }

    #[tokio::test]
    async fn test_private_addresses_are_not_looked_up() {
        // Would otherwise make a network request
        assert_eq!(None, GeoIp::new().lookup(ip("192.168.1.1")).await);
    }
}
