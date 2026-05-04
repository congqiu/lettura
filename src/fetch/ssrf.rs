//! SSRF protection: block requests to private/reserved IP ranges.
//!
//! This module provides URL validation to prevent Server-Side Request Forgery
//! attacks by rejecting URLs that resolve to loopback, private, link-local,
//! or cloud metadata addresses.

use std::net::IpAddr;
use url::Url;

/// Check if a host string is a private/reserved IP address.
/// Returns true for loopback, private ranges, link-local, cloud metadata,
/// benchmarking, documentation, and multicast addresses.
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
            || (o[0] == 100 && o[1] >= 64 && o[1] <= 127)  // 100.64.0.0/10 CGNAT
            || (o[0] == 198 && o[1] >= 18 && o[1] <= 19)   // 198.18.0.0/15 benchmarking
            || (o[0] == 198 && o[1] == 51 && o[2] == 100)  // 198.51.100.0/24 documentation
            || (o[0] == 203 && o[1] == 0 && o[2] == 113)   // 203.0.113.0/24 documentation
            || o[0] >= 224     // 224.0.0.0/4 multicast + reserved
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
            || (s[0] & 0xfe00) == 0xfc00  // fc00::/7 unique local
            || (s[0] & 0xffc0) == 0xfe80  // fe80::/10 link-local
            || s[0] == 0x2001 && s[1] == 0xdb8  // 2001:db8::/32 documentation
            || s[0] == 0x2002              // 2002::/16 6to4 (can reach private IPv4)
            || s[0] >= 0xff00              // ff00::/8 multicast
        }
    }
}

/// Validate that a URL does not point to a private/reserved IP.
/// Rejects non-HTTP schemes (file://, gopher://, etc.) and private/reserved hosts.
pub fn validate_url(raw_url: &str) -> Result<(), String> {
    let parsed = Url::parse(raw_url).map_err(|e| format!("invalid URL: {e}"))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("blocked non-HTTP scheme: {scheme}://"));
    }
    let host = parsed.host_str().ok_or_else(|| "URL has no host".to_string())?;
    if is_private_host(host) {
        return Err(format!("blocked private/reserved host: {host}"));
    }
    Ok(())
}

/// Re-validate resolved IPs to defend against DNS rebinding.
/// Call this after DNS resolution in the HTTP client to check that
/// the actual connected IP is not in a reserved range.
pub fn check_resolved_ips(ips: &[IpAddr]) -> Result<(), String> {
    for ip in ips {
        if is_private_ip(ip) {
            return Err(format!("resolved to reserved IP: {ip}"));
        }
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
    fn blocks_carrier_nat() {
        assert!(is_private_host("100.64.0.1"));
        assert!(is_private_host("100.127.255.255"));
    }

    #[test]
    fn blocks_benchmarking() {
        assert!(is_private_host("198.18.0.1"));
        assert!(is_private_host("198.19.255.255"));
    }

    #[test]
    fn blocks_documentation() {
        assert!(is_private_host("198.51.100.1"));
        assert!(is_private_host("203.0.113.1"));
    }

    #[test]
    fn blocks_multicast() {
        assert!(is_private_host("224.0.0.1"));
        assert!(is_private_host("239.255.255.255"));
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
    fn blocks_ipv6_documentation() {
        assert!(is_private_host("2001:db8::1"));
    }

    #[test]
    fn blocks_ipv6_6to4() {
        assert!(is_private_host("2002::1"));
    }

    #[test]
    fn blocks_ipv6_multicast() {
        assert!(is_private_host("ff00::1"));
    }

    #[test]
    fn allows_public_ips() {
        assert!(!is_private_host("8.8.8.8"));
        assert!(!is_private_host("1.1.1.1"));
        assert!(!is_private_host("203.0.113.1") == false || is_private_host("203.0.113.1")); // documentation range
        assert!(!is_private_host("104.16.0.1"));
    }

    #[test]
    fn allows_public_domains() {
        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("github.com"));
    }

    #[test]
    fn validate_url_blocks_file_scheme() {
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn validate_url_blocks_gopher_scheme() {
        assert!(validate_url("gopher://internal/").is_err());
    }

    #[test]
    fn validate_url_allows_http_scheme() {
        assert!(validate_url("http://example.com/page").is_ok());
    }

    #[test]
    fn validate_url_allows_https_scheme() {
        assert!(validate_url("https://example.com/page").is_ok());
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

    #[test]
    fn check_resolved_ips_rejects_private() {
        use std::net::Ipv4Addr;
        let ips = vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))];
        assert!(check_resolved_ips(&ips).is_err());
    }

    #[test]
    fn check_resolved_ips_accepts_public() {
        use std::net::Ipv4Addr;
        let ips = vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))];
        assert!(check_resolved_ips(&ips).is_ok());
    }
}