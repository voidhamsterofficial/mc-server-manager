//! Resolving how a server is actually reached: the port it listens on (read
//! from whichever config file its software uses) and this machine's LAN IP.
//! Kept out of the command layer so the address logic is unit-testable and the
//! commands stay thin.

use std::path::Path;

use crate::portforward;
use crate::servers::Loader;
use crate::storage::properties;

/// The port players actually connect on. Proxies don't have a
/// `server.properties` — they keep their listen port in their own config — so
/// each family is read from the right file, falling back to its default.
pub fn configured_port(server_dir: &Path, loader: Loader) -> String {
    let from_config = match loader {
        Loader::Velocity => velocity_port(server_dir),
        Loader::BungeeCord => bungee_port(server_dir),
        _ => properties_port(server_dir),
    };
    from_config.unwrap_or_else(|| default_port(loader).to_string())
}

/// Vanilla and every Java server default to 25565; the proxies to 25577.
pub fn default_port(loader: Loader) -> &'static str {
    if loader.is_proxy() {
        "25577"
    } else {
        "25565"
    }
}

fn is_port(text: &str) -> bool {
    !text.is_empty() && text.parse::<u16>().is_ok()
}

fn properties_port(server_dir: &Path) -> Option<String> {
    properties::read(server_dir)
        .ok()?
        .into_iter()
        .find(|property| property.key == "server-port")
        .map(|property| property.value)
        .filter(|value| is_port(value))
}

/// Velocity's `velocity.toml` holds `bind = "0.0.0.0:25577"`.
fn velocity_port(server_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(server_dir.join("velocity.toml")).ok()?;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != "bind" {
            continue;
        }
        let port = value.trim().trim_matches('"').rsplit(':').next()?;
        if is_port(port) {
            return Some(port.to_string());
        }
    }
    None
}

/// BungeeCord's `config.yml` lists `host: 0.0.0.0:25577` under `listeners`.
fn bungee_port(server_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(server_dir.join("config.yml")).ok()?;

    for line in text.lines() {
        let trimmed = line.trim().trim_start_matches("- ").trim();
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if key.trim() != "host" {
            continue;
        }
        let port = value
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .rsplit(':')
            .next()?;
        if is_port(port) {
            return Some(port.to_string());
        }
    }
    None
}

/// This machine's LAN IP, found by asking the OS which local address it
/// would use to reach the internet (no packet is actually sent).
pub fn local_lan_ip() -> String {
    let fallback = "127.0.0.1".to_string();
    let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") else {
        return fallback;
    };
    if socket.connect("8.8.8.8:80").is_err() {
        return fallback;
    }
    match socket.local_addr() {
        Ok(address) => address.ip().to_string(),
        Err(_) => fallback,
    }
}

/// UPnP protocol a server needs: Bedrock is UDP, every Java flavour is TCP.
pub fn forward_protocol(loader: Loader) -> portforward::Protocol {
    if loader == Loader::Bds {
        portforward::Protocol::Udp
    } else {
        portforward::Protocol::Tcp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `contents` to `name` in a fresh temp dir and returns the dir.
    fn dir_with(name: &str, contents: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("blockparty-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join(name), contents).expect("write config");
        dir
    }

    #[test]
    fn reads_velocity_port_from_its_toml() {
        let dir = dir_with(
            "velocity.toml",
            "# What port should the proxy be bound to?\nbind = \"0.0.0.0:25599\"\nmotd = \"hi\"\n",
        );
        assert_eq!(configured_port(&dir, Loader::Velocity), "25599");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reads_bungee_port_from_its_yaml() {
        let dir = dir_with(
            "config.yml",
            "listeners:\n- query_port: 25577\n  host: 0.0.0.0:25588\n  motd: 'hi'\n",
        );
        assert_eq!(configured_port(&dir, Loader::BungeeCord), "25588");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn proxies_fall_back_to_the_proxy_default_not_25565() {
        // An empty dir: a proxy must not report the vanilla port.
        let dir = dir_with("unrelated.txt", "");
        assert_eq!(configured_port(&dir, Loader::Velocity), "25577");
        assert_eq!(configured_port(&dir, Loader::BungeeCord), "25577");
        assert_eq!(configured_port(&dir, Loader::Vanilla), "25565");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn commented_out_bind_lines_are_ignored() {
        let dir = dir_with(
            "velocity.toml",
            "# bind = \"0.0.0.0:11111\"\nbind = \"0.0.0.0:25577\"\n",
        );
        assert_eq!(configured_port(&dir, Loader::Velocity), "25577");
        let _ = std::fs::remove_dir_all(dir);
    }
}
