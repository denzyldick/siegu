//! Built-in TURN relay (#16/#17).
//!
//! Siegu ships a tiny TURN server inside the host app instead of requiring a
//! separate coturn install. When enabled it listens for UDP TURN traffic and
//! relays WebRTC media between the host and guests that cannot reach each other
//! directly (e.g. a guest on mobile data). The host's own `rtc_configuration`
//! advertises the relay to its peers, and the served guest page is told about it
//! via `window.sieguTurnConfig` ([`crate::lan_server`]).
//!
//! Credentials use long-term TURN auth (RFC 5389) against a fixed
//! username/password pair from the app settings (`turn_username` /
//! `turn_password`). The relay never stores or inspects payloads: it only
//! forwards datagrams between the host and one guest (rendezvous).

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use turn::auth::AuthHandler;
use turn::relay::relay_static::RelayAddressGeneratorStatic;
use turn::server::config::{ConnConfig, ServerConfig};
use util::vnet::net::Net;
use util::Conn;

pub const DEFAULT_REALM: &str = "siegu";

/// Credentials handed to ICE so both sides can reach the embedded relay.
#[derive(Debug, Clone)]
pub struct TurnCredentials {
    /// TURN URL advertised to peers, e.g. `turn:192.168.1.5:51820?transport=udp`.
    pub url: String,
    pub username: String,
    pub password: String,
}

/// A running embedded TURN server. Dropping it stops the relay.
pub struct EmbeddedTurn {
    creds: TurnCredentials,
    server: Option<turn::server::Server>,
    _conn: Arc<UdpSocket>,
}

impl EmbeddedTurn {
    /// ICE-relevant TURN settings (URL + long-term credentials).
    pub fn credentials(&self) -> &TurnCredentials {
        &self.creds
    }

    /// Gracefully stop the relay (also happens implicitly on drop while the
    /// tokio runtime the relay was started on is still alive).
    pub async fn close(&mut self) {
        if let Some(server) = self.server.take() {
            let _ = server.close().await;
        }
    }
}

/// Validates long-term TURN credentials against the configured static pair.
struct StaticAuthHandler {
    cred_map: HashMap<String, Vec<u8>>,
}

impl StaticAuthHandler {
    fn new(username: &str, realm: &str, password: &str) -> Self {
        let mut cred_map = HashMap::new();
        cred_map.insert(
            username.to_owned(),
            turn::auth::generate_auth_key(username, realm, password),
        );
        StaticAuthHandler { cred_map }
    }
}

impl AuthHandler for StaticAuthHandler {
    fn auth_handle(
        &self,
        username: &str,
        _realm: &str,
        _src_addr: SocketAddr,
    ) -> Result<Vec<u8>, turn::Error> {
        self.cred_map
            .get(username)
            .cloned()
            .ok_or_else(|| turn::Error::Other(format!("no such user: {username}")))
    }
}

/// Resolve the relay address advertised to peers.
///
/// By default the host's LAN address is used, which is correct for in-LAN
/// guests. For guests on mobile data the router must forward UDP to the TURN
/// port; set `turn_public_host` to the outward-routable IP in that case.
fn relay_address(public_host: Option<&str>) -> IpAddr {
    public_host
        .filter(|h| !h.trim().is_empty())
        .and_then(|h| h.trim().parse::<IpAddr>().ok())
        .or_else(|| crate::mdns::local_ip().ok())
        .unwrap_or_else(|| IpAddr::from([127, 0, 0, 1]))
}

/// Start the embedded TURN server on `port` (`0` = auto-assigned), authenticating
/// `username`/`password` under `realm`. Returns the running server plus the
/// credentials (actual port included) peers should use.
///
/// The relay binds `0.0.0.0`, so on Linux no privileged port is required and
/// any local interface can serve guests. `public_host`, when given, is
/// advertised (both in the TURN URL and as the relay address) so internet
/// guests know where to reach the relay behind the router's forward.
pub async fn start(
    port: u16,
    username: &str,
    password: &str,
    realm: &str,
    public_host: Option<&str>,
) -> Result<EmbeddedTurn, Box<dyn std::error::Error + Send + Sync>> {
    if username.trim().is_empty() || password.is_empty() {
        return Err("turn_username and turn_password must be set to start the relay".into());
    }

    let socket = Arc::new(UdpSocket::bind(("0.0.0.0", port)).await?);
    let actual_port = socket.local_addr()?.port();

    let relay_ip = relay_address(public_host);
    let url_host = public_host
        .filter(|h| !h.trim().is_empty())
        .map(|h| h.trim().to_string())
        .unwrap_or_else(|| relay_ip.to_string());
    let url = format!("turn:{url_host}:{actual_port}?transport=udp");

    let conn: Arc<dyn Conn + Send + Sync> = socket.clone();

    let server = turn::server::Server::new(ServerConfig {
        conn_configs: vec![ConnConfig {
            conn,
            relay_addr_generator: Box::new(RelayAddressGeneratorStatic {
                relay_address: relay_ip,
                address: "0.0.0.0".to_owned(),
                net: Arc::new(Net::new(None)),
            }),
        }],
        realm: realm.to_owned(),
        auth_handler: Arc::new(StaticAuthHandler::new(username, realm, password)),
        channel_bind_timeout: Duration::from_secs(600),
        alloc_close_notify: None,
    })
    .await?;

    Ok(EmbeddedTurn {
        creds: TurnCredentials {
            url,
            username: username.to_owned(),
            password: password.to_owned(),
        },
        server: Some(server),
        _conn: socket,
    })
}

/// Start the relay when the app settings ask for it, then publish the ICE
/// settings to this process so [`crate::mesh_transport`] and the served guest
/// page pick them up via `SIEGU_TURN_*`. `Ok(None)` means "no relay needed":
/// either `turn_enabled` is false, or an external TURN is already configured
/// through `SIEGU_TURN_URLS`.
///
/// Requires the runtime-side caller to keep the returned handle alive for the
/// process lifetime.
pub async fn ensure_started(
    config_path: &str,
) -> Result<Option<EmbeddedTurn>, Box<dyn std::error::Error + Send + Sync>> {
    if std::env::var("SIEGU_TURN_URLS")
        .ok()
        .is_some_and(|v| !v.trim().is_empty())
    {
        tracing::info!(
            "external TURN already configured via SIEGU_TURN_URLS; embedded relay disabled"
        );
        return Ok(None);
    }

    let db = crate::database::Database::new(config_path);
    let state = db.get_state();
    if state.get("turn_enabled").map(String::as_str) != Some("true") {
        return Ok(None);
    }

    let port: u16 = state
        .get("turn_port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);

    let (username, password) = match (
        state.get("turn_username").map(String::as_str),
        state.get("turn_password").map(String::as_str),
    ) {
        (Some(u), Some(p)) if !u.trim().is_empty() && !p.is_empty() => {
            (u.trim().to_string(), p.to_string())
        }
        (Some(u), Some(_)) if !u.trim().is_empty() => (u.trim().to_string(), random_password()),
        _ => (random_username(), random_password()),
    };

    // Persisting lets the host expose the same relay to every guest session
    // for the app's lifetime and keeps a stable URL across restarts.
    let mut next = state.clone();
    next.insert("turn_username".to_string(), username.clone());
    next.insert("turn_password".to_string(), password.clone());
    db.set_state(next);

    let public_host = state.get("turn_public_host").cloned();
    let turn = start(
        port,
        &username,
        &password,
        DEFAULT_REALM,
        public_host.as_deref(),
    )
    .await?;

    // Publish so the host's rtc_configuration() and lan_server pick this up.
    std::env::set_var("SIEGU_TURN_URLS", turn.credentials().url.clone());
    std::env::set_var("SIEGU_TURN_USERNAME", turn.credentials().username.clone());
    std::env::set_var("SIEGU_TURN_CREDENTIAL", turn.credentials().password.clone());

    tracing::info!(
        url = %turn.credentials().url,
        "embedded TURN relay started"
    );
    Ok(Some(turn))
}

/// Random relay credential for setups that enable the relay without picking
/// their own (settings keep it visible for copy/paste into other devices).
fn random_password() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn random_username() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("siegu-{}", rng.gen_range(100_000..999_999))
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::sync::Arc;

    use tokio::net::UdpSocket;
    use turn::client::{Client, ClientConfig};

    use super::*;

    fn port_from(url: &str) -> u16 {
        url.rsplit(':')
            .next()
            .and_then(|p| p.split('?').next())
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn test_ensure_started_disabled_returns_none(
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().display().to_string();
        let db = crate::database::Database::new(&path);
        let mut state = std::collections::HashMap::new();
        state.insert("turn_enabled".to_string(), "false".to_string());
        db.set_state(state);
        assert!(super::ensure_started(&path).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_started_enables_relay_from_config(
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if std::env::var("SIEGU_TURN_URLS")
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
        {
            return Ok(());
        }
        let dir = tempfile::tempdir()?;
        let path = dir.path().display().to_string();
        let db = crate::database::Database::new(&path);
        let mut state = std::collections::HashMap::new();
        state.insert("turn_enabled".to_string(), "true".to_string());
        state.insert("turn_username".to_string(), "host1".to_string());
        state.insert("turn_password".to_string(), "pw".to_string());
        db.set_state(state);

        let turn = super::ensure_started(&path).await?.expect("relay started");
        assert!(
            turn.credentials().url.starts_with("turn:"),
            "url {}",
            turn.credentials().url
        );
        assert_eq!(turn.credentials().username, "host1");
        assert_eq!(
            std::env::var("SIEGU_TURN_USERNAME").ok().as_deref(),
            Some("host1"),
            "host ICE config must see the relay"
        );
        assert!(std::env::var("SIEGU_TURN_CREDENTIAL").ok().as_deref() == Some("pw"));

        std::env::remove_var("SIEGU_TURN_URLS");
        std::env::remove_var("SIEGU_TURN_USERNAME");
        std::env::remove_var("SIEGU_TURN_CREDENTIAL");
        Ok(())
    }

    #[tokio::test]
    async fn test_start_advertises_actual_port(
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let turn = super::start(0, "alice", "secret", DEFAULT_REALM, Some("127.0.0.1")).await?;
        assert!(turn.credentials().url.starts_with("turn:127.0.0.1:"));
        assert_eq!(turn.credentials().username, "alice");
        assert!(turn.server.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn test_client_allocation_roundtrip(
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut turn = super::start(0, "alice", "secret", DEFAULT_REALM, Some("127.0.0.1")).await?;
        let port = port_from(&turn.credentials().url);

        let conn = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        let client = Client::new(ClientConfig {
            stun_serv_addr: format!("127.0.0.1:{port}"),
            turn_serv_addr: format!("127.0.0.1:{port}"),
            username: "alice".to_owned(),
            password: "secret".to_owned(),
            realm: String::new(),
            software: String::new(),
            rto_in_ms: 0,
            conn,
            vnet: None,
        })
        .await?;

        client.listen().await?;
        client.allocate().await?;

        let allocations = turn
            .server
            .as_ref()
            .expect("server running")
            .get_allocations_info(None)
            .await?;
        assert_eq!(allocations.len(), 1, "exactly one allocation");
        let info = allocations.values().next().expect("one allocation");
        assert_eq!(
            IpAddr::from_str(&info.relay_addr.ip().to_string()).ok(),
            Some(relay_address(Some("127.0.0.1"))),
            "relay advertises the configured address"
        );

        client.close().await?;
        turn.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wrong_password_rejected() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    {
        let port = {
            let turn = super::start(0, "alice", "secret", DEFAULT_REALM, Some("127.0.0.1")).await?;
            port_from(&turn.credentials().url)
        };

        let conn = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        let client = Client::new(ClientConfig {
            stun_serv_addr: format!("127.0.0.1:{port}"),
            turn_serv_addr: format!("127.0.0.1:{port}"),
            username: "alice".to_owned(),
            password: "WRONG".to_owned(),
            realm: String::new(),
            software: String::new(),
            rto_in_ms: 300,
            conn,
            vnet: None,
        })
        .await?;

        client.listen().await?;
        let result = client.allocate().await;
        assert!(
            result.is_err(),
            "allocation must fail with the wrong password"
        );
        Ok(())
    }
}
