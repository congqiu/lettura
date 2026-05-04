//! SSRF protection: block requests to private/reserved IP ranges.
//!
//! This module provides URL validation to prevent Server-Side Request Forgery
//! attacks by rejecting URLs that resolve to loopback, private, link-local,
//! or cloud metadata addresses.

use std::net::IpAddr;
use url::Url;

/// Check if a host string is a private/reserved IP address.
/// Returns true for loopback, private ranges, link-local, cloud metadata.
/// Returns false for domain names (they'll be resolved and checked later).
pub fn is_private_host(host: &str) -> bool {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_private_ip(&ip);
    }
    false
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 0           // 0.0.0.0/8
            || o[0] == 127      // 127.0.0.0/8
            || o[0] == 10       // 10.0.0.0/8
            || (o[0] == 172 && o[1] >= 16 && o[1] <= 31)  // 172.16.0.0/12
            || (o[0] == 192 && o[1] == 168)                // 192.168.0.0/16
            || (o[0] == 169 && o[1] == 254)                // 169.254.0.0/16
            || (o[0] == 100 && o[1] >= 64 && o[1] <= 127)  // 100.64.0.0/10
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
            || (v6.segments()[0] & 0xfe00) == 0xfc00  // fc00::/7 unique local
            || (v6.segments()[0] & 0xffc0) == 0xfe80  // fe80::/10 link-local
        }
    }
}

/// Validate that a URL does not point to a private/reserved IP.
pub fn validate_url(raw_url: &str) -> Result<(), String> {
    let parsed = Url::parse(raw_url).map_err(|e| format!("invalid URL: {e}"))?;
    let host = parsed.host_str().ok_or_else(|| "URL has no host".to_string())?;
    if is_private_host(host) {
        return Err(format!("blocked private/reserved host: {host}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_loopback_ipv4() {
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("127.0.0.100"));
        assert!(is_private_host("0.0.0.0"));
    }

    #[test]
    fn blocks_private_class_a() {
        assert!(is_private_host("10.0.0.1"));
        assert!(is_private_host("10.255.255.255"));
    }

    #[test]
    fn blocks_private_class_b() {
        assert!(is_private_host("172.16.0.1"));
        assert!(is_private_host("172.31.255.255"));
    }

    #[test]
    fn blocks_private_class_c() {
        assert!(is_private_host("192.168.0.1"));
        assert!(is_private_host("192.168.1.100"));
    }

    #[test]
    fn blocks_link_local() {
        assert!(is_private_host("169.254.169.254"));
        assert!(is_private_host("169.254.0.1"));
    }

    #[test]
    fn blocks_ipv6_loopback() {
        assert!(is_private_host("::1"));
    }

    #[test]
    fn blocks_ipv6_unique_local() {
        assert!(is_private_host("fc00::1"));
        assert!(is_private_host("fd12:3456::1"));
    }

    #[test]
    fn blocks_ipv6_link_local() {
        assert!(is_private_host("fe80::1"));
    }

    #[test]
    fn allows_public_ips() {
        assert!(!is_private_host("8.8.8.8"));
        assert!(!is_private_host("1.1.1.1"));
        assert!(!is_private_host("203.0.113.1"));
    }

    #[test]
    fn allows_public_domains() {
        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("github.com"));
    }

    #[test]
    fn validate_url_blocks_private_ip() {
        assert!(validate_url("http://127.0.0.1/admin").is_err());
        assert!(validate_url("http://10.0.0.1/secret").is_err());
        assert!(validate_url("http://192.168.1.1/router").is_err());
        assert!(validate_url("http://169.254.169.254/metadata").is_err());
    }

    #[test]
    fn validate_url_allows_public() {
        assert!(validate_url("https://example.com/page").is_ok());
        assert!(validate_url("http://8.8.8.8/dns").is_ok());
    }
}
