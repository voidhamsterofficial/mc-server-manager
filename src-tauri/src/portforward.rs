//! Automatic port forwarding over UPnP-IGD.
//!
//! Most home routers speak UPnP, which lets an app on the LAN ask the router to
//! open and forward a port — no manual router config. It isn't universal: UPnP
//! is sometimes disabled, and if the ISP uses carrier-grade NAT (CGNAT) no
//! router mapping can be reached from the internet at all. We detect that case
//! so the UI can explain it instead of silently "succeeding".

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use igd_next::aio::tokio::search_gateway;
use igd_next::{PortMappingProtocol, SearchOptions};

/// Shown in the router's UPnP mapping table.
const DESCRIPTION: &str = "Blockparty Minecraft server";
/// Ask for an indefinite lease; fall back to a week if the router refuses one.
const INDEFINITE_LEASE: u32 = 0;
const FALLBACK_LEASE: u32 = 604_800;

/// Which transport the game uses: Java is TCP, Bedrock is UDP.
#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Protocol {
    fn as_igd(self) -> PortMappingProtocol {
        match self {
            Protocol::Tcp => PortMappingProtocol::TCP,
            Protocol::Udp => PortMappingProtocol::UDP,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PortForwardError {
    #[error("no UPnP gateway found")]
    NoGateway,
    #[error("{0}")]
    Router(String),
}

/// Asks the router to forward `port` to this machine and returns the router's
/// WAN (external) IP so the caller can check reachability.
pub async fn open(
    protocol: Protocol,
    port: u16,
    lan_ip: Ipv4Addr,
) -> Result<IpAddr, PortForwardError> {
    let gateway = search_gateway(SearchOptions::default())
        .await
        .map_err(|_| PortForwardError::NoGateway)?;

    let local = SocketAddr::new(IpAddr::V4(lan_ip), port);
    let proto = protocol.as_igd();

    // Some routers only accept finite leases, so retry with one if the
    // indefinite request is rejected.
    if gateway
        .add_port(proto, port, local, INDEFINITE_LEASE, DESCRIPTION)
        .await
        .is_err()
    {
        gateway
            .add_port(proto, port, local, FALLBACK_LEASE, DESCRIPTION)
            .await
            .map_err(|error| PortForwardError::Router(error.to_string()))?;
    }

    gateway
        .get_external_ip()
        .await
        .map_err(|error| PortForwardError::Router(error.to_string()))
}

/// Removes a mapping previously added by [`open`]. A missing mapping is fine.
pub async fn close(protocol: Protocol, port: u16) -> Result<(), PortForwardError> {
    let gateway = search_gateway(SearchOptions::default())
        .await
        .map_err(|_| PortForwardError::NoGateway)?;
    gateway
        .remove_port(protocol.as_igd(), port)
        .await
        .map_err(|error| PortForwardError::Router(error.to_string()))
}

/// This machine's real public IP, as seen from the internet — used to tell
/// whether the router's WAN IP is actually reachable (CGNAT check).
pub async fn public_ip() -> Option<String> {
    let response = reqwest::get("https://api.ipify.org").await.ok()?;
    let text = response.text().await.ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.parse::<IpAddr>().is_err() {
        return None;
    }
    Some(trimmed.to_string())
}

/// True when the router's WAN address is itself private or in the CGNAT range
/// (100.64.0.0/10), meaning the ISP puts the router behind another NAT — so a
/// port mapping on this router can never be reached from the public internet.
pub fn is_behind_carrier_nat(wan_ip: IpAddr) -> bool {
    match wan_ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || is_cgnat(v4),
        // Public IPv6 has no NAT; treat it as directly reachable.
        IpAddr::V6(_) => false,
    }
}

/// The shared-address space reserved for carrier-grade NAT: 100.64.0.0/10.
fn is_cgnat(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (64..=127).contains(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn flags_private_and_cgnat_wan_addresses() {
        assert!(is_behind_carrier_nat(v4("192.168.0.1")));
        assert!(is_behind_carrier_nat(v4("10.0.0.4")));
        assert!(is_behind_carrier_nat(v4("172.16.5.9")));
        // CGNAT 100.64.0.0/10
        assert!(is_behind_carrier_nat(v4("100.64.0.1")));
        assert!(is_behind_carrier_nat(v4("100.127.255.254")));
    }

    #[test]
    fn allows_ordinary_public_addresses() {
        assert!(!is_behind_carrier_nat(v4("8.8.8.8")));
        assert!(!is_behind_carrier_nat(v4("100.63.0.1"))); // just below the CGNAT block
        assert!(!is_behind_carrier_nat(v4("100.128.0.1"))); // just above it
        assert!(!is_behind_carrier_nat(v4("203.0.113.7")));
    }
}
