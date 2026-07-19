//! Automatic port forwarding over UPnP-IGD.
//!
//! Most home routers speak UPnP, which lets an app on the LAN ask the router to
//! open and forward a port — no manual router config. It isn't universal: UPnP
//! is sometimes disabled, and if the ISP uses carrier-grade NAT (CGNAT) no
//! router mapping can be reached from the internet at all. We detect that case
//! so the UI can explain it instead of silently "succeeding".

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use igd_next::aio::tokio::{search_gateway, Tokio};
use igd_next::aio::Gateway;
use igd_next::{PortMappingProtocol, SearchOptions};

/// Shown in the router's UPnP mapping table.
const DESCRIPTION: &str = "Blockparty Minecraft server";
/// Ask for an indefinite lease; fall back to a week if the router refuses one.
const INDEFINITE_LEASE: u32 = 0;
const FALLBACK_LEASE: u32 = 604_800;
/// How long to wait for a router to answer the SSDP search.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(5);

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

/// Finds the router's UPnP gateway, searching from this machine's LAN
/// interface first.
///
/// This matters more than it looks: the default bind address is `0.0.0.0`,
/// which lets the OS choose which interface the SSDP multicast leaves by.
/// Machines with virtual adapters — Hyper-V, WSL, VirtualBox, VPNs, which is
/// most Windows machines — routinely send it out one of those instead of the
/// real network, so a router that speaks UPnP perfectly well is never found.
/// Binding to the actual LAN address aims the search at the right network; we
/// still fall back to the default in case that address can't be bound.
async fn find_gateway(lan_ip: Ipv4Addr) -> Result<Gateway<Tokio>, PortForwardError> {
    let from_lan_interface = SearchOptions {
        bind_addr: SocketAddr::new(IpAddr::V4(lan_ip), 0),
        timeout: Some(SEARCH_TIMEOUT),
        ..Default::default()
    };
    if let Ok(gateway) = search_gateway(from_lan_interface).await {
        return Ok(gateway);
    }

    let any_interface = SearchOptions {
        timeout: Some(SEARCH_TIMEOUT),
        ..Default::default()
    };
    search_gateway(any_interface)
        .await
        .map_err(|_| PortForwardError::NoGateway)
}

/// Adds the mapping, working around the two ways routers commonly refuse one,
/// and returns the external port that ended up mapped — normally `port`, but
/// see the fallback below.
async fn add_mapping(
    gateway: &Gateway<Tokio>,
    proto: PortMappingProtocol,
    port: u16,
    local: SocketAddr,
) -> Result<u16, PortForwardError> {
    // Some routers only accept finite leases, so retry with one if the
    // indefinite request is rejected.
    if gateway
        .add_port(proto, port, local, INDEFINITE_LEASE, DESCRIPTION)
        .await
        .is_ok()
    {
        return Ok(port);
    }
    if gateway
        .add_port(proto, port, local, FALLBACK_LEASE, DESCRIPTION)
        .await
        .is_ok()
    {
        return Ok(port);
    }

    // A leftover mapping for this port — ours from a previous run, or one the
    // router kept after a reboot — makes it reject the new one. Clear it and
    // try once more.
    let _ = gateway.remove_port(proto, port).await;
    if gateway
        .add_port(proto, port, local, FALLBACK_LEASE, DESCRIPTION)
        .await
        .is_ok()
    {
        return Ok(port);
    }

    // Some routers refuse to delete a mapping unless the request comes from
    // the internal client that created it — e.g. this machine after a DHCP
    // renewal changed its LAN IP, or another device that's no longer even on
    // the network. There's no way to force that deletion from here, so
    // instead of fighting the router for `port`, ask it to hand back any free
    // external port. Players just need host:port, and that port doesn't need
    // to match the server's internal port.
    gateway
        .add_any_port(proto, local, FALLBACK_LEASE, DESCRIPTION)
        .await
        .map_err(|error| PortForwardError::Router(error.to_string()))
}

/// Asks the router to forward `port` to this machine. Returns the router's WAN
/// (external) IP and the external port actually mapped — usually `port`, but a
/// different one if the router wouldn't give up a stale conflicting mapping on
/// `port` (see [`add_mapping`]).
pub async fn open(
    protocol: Protocol,
    port: u16,
    lan_ip: Ipv4Addr,
) -> Result<(IpAddr, u16), PortForwardError> {
    let gateway = find_gateway(lan_ip).await?;
    let local = SocketAddr::new(IpAddr::V4(lan_ip), port);

    let external_port = add_mapping(&gateway, protocol.as_igd(), port, local).await?;

    let wan_ip = gateway
        .get_external_ip()
        .await
        .map_err(|error| PortForwardError::Router(error.to_string()))?;
    Ok((wan_ip, external_port))
}

/// Removes a mapping previously added by [`open`]. `external_port` is the port
/// [`open`] returned, which may differ from the server's internal port. A
/// missing mapping is fine.
pub async fn close(
    protocol: Protocol,
    external_port: u16,
    lan_ip: Ipv4Addr,
) -> Result<(), PortForwardError> {
    let gateway = find_gateway(lan_ip).await?;
    gateway
        .remove_port(protocol.as_igd(), external_port)
        .await
        .map_err(|error| PortForwardError::Router(error.to_string()))
}

/// How many mapping slots to walk before giving up. Routers hold far fewer
/// than this; the bound just stops a misbehaving one from looping forever.
const MAX_MAPPING_SCAN: u32 = 128;

/// Whether this machine already has `internal_port` forwarded to it — i.e. a
/// mapping we (or a previous run of the app) left on the router. Returns the
/// router's WAN IP and the external port the mapping actually uses (which,
/// per [`open`]'s fallback, may not equal `internal_port`), so the caller can
/// rebuild the shareable address.
///
/// Mappings outlive the app, so this is how a restart rediscovers that a
/// server is still open to the internet instead of claiming it isn't.
/// Matching by internal port/client rather than by external port also means a
/// previous fallback mapping (opened on a different external port) is still
/// found.
pub async fn status(
    protocol: Protocol,
    internal_port: u16,
    lan_ip: Ipv4Addr,
) -> Result<Option<(IpAddr, u16)>, PortForwardError> {
    let gateway = find_gateway(lan_ip).await?;
    let wanted = protocol.as_igd();
    let us = lan_ip.to_string();

    // Routers expose mappings only by index, so walk them until the index runs
    // out (the documented way to enumerate).
    for index in 0..MAX_MAPPING_SCAN {
        let Ok(entry) = gateway.get_generic_port_mapping_entry(index).await else {
            break;
        };
        let is_ours = entry.internal_port == internal_port
            && entry.protocol == wanted
            && entry.internal_client == us;
        if is_ours {
            let wan_ip = gateway
                .get_external_ip()
                .await
                .map_err(|error| PortForwardError::Router(error.to_string()))?;
            return Ok(Some((wan_ip, entry.external_port)));
        }
    }
    Ok(None)
}

/// This machine's real public IPv4, as seen from the internet — used to tell
/// whether the router's WAN IP is actually reachable (CGNAT check).
///
/// Deliberately the IPv4-only endpoint: the dual-stack one answers with an
/// IPv6 address whenever the machine has IPv6, which never matches the
/// router's IPv4 WAN address and would make every such setup look like CGNAT.
pub async fn public_ip() -> Option<String> {
    let response = reqwest::get("https://api4.ipify.org").await.ok()?;
    let text = response.text().await.ok()?;
    let trimmed = text.trim();
    if trimmed.parse::<Ipv4Addr>().is_err() {
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
