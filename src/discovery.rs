use anyhow::Result;
use iroh::EndpointId;
use log::{info, trace, warn};
use std::net::SocketAddr;
use swarm_discovery::Discoverer;
use tokio::sync::mpsc;

/// Information about a discovered peer on the local network.
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub endpoint_id: EndpointId,
    pub addrs: Vec<SocketAddr>,
}

/// Encode an EndpointId as a short string suitable for mDNS peer IDs.
/// The SRV target label is "{peer_id}-{port}.local." so peer_id + 6 chars
/// (dash + 5 digit port) must fit in a 63-byte DNS label.
/// 26 bytes of hex = 52 chars, leaving room for "-65535" (6 chars) = 58 total.
fn encode_peer_id(id: &EndpointId) -> String {
    let bytes = id.as_bytes();
    bytes[..26]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// Decode a peer ID string back to an EndpointId.
/// Since we truncated to 31 bytes, we need the full key from the caller.
/// Instead, we store the full hex in a TXT attribute and use a short ID for the mDNS name.
fn decode_peer_id(hex: &str) -> Option<EndpointId> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for i in 0..32 {
        bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    let key = iroh::PublicKey::from_bytes(&bytes).ok()?;
    Some(key)
}

/// Start mDNS discovery, broadcasting this node's presence and discovering others.
///
/// Returns a channel receiver that yields newly discovered peers and a drop guard
/// that stops discovery when dropped.
///
/// If `broadcast_port` is `Some(port)`, the node will advertise itself on the LAN.
/// If `None`, the node will only discover others (no broadcast).
pub fn start_discovery(
    service_name: String,
    endpoint_id: EndpointId,
    broadcast_port: Option<u16>,
    runtime_handle: &tokio::runtime::Handle,
) -> Result<(mpsc::Receiver<DiscoveredPeer>, swarm_discovery::DropGuard)> {
    let (tx, rx) = mpsc::channel::<DiscoveredPeer>(64);

    let short_id = encode_peer_id(&endpoint_id);
    let full_hex = endpoint_id.to_string();
    let mut builder = Discoverer::new_interactive(service_name, short_id);

    if let Some(port) = broadcast_port {
        let addrs: Vec<std::net::IpAddr> = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .filter(|iface| !iface.is_loopback())
            .map(|iface| iface.addr.ip())
            .collect();

        info!(
            "Broadcasting on mDNS: port={}, addrs={:?}",
            port,
            addrs.iter().map(|a| a.to_string()).collect::<Vec<_>>()
        );
        builder = builder.with_addrs(port, addrs);

        // Store the full endpoint ID in a TXT attribute so peers can reconstruct it
        builder = builder
            .with_txt_attributes(vec![("eid".to_string(), Some(full_hex))])
            .expect("valid TXT attributes");
    }

    let own_id = endpoint_id;
    let seen = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::<EndpointId>::new()));
    builder = builder.with_callback(move |_peer_id_str, peer| {
        let full_hex: String = match peer.txt_attribute("eid") {
            Some(Some(hex)) => hex.to_string(),
            _ => return,
        };

        let endpoint_id = match decode_peer_id(&full_hex) {
            Some(id) => id,
            None => {
                warn!("Failed to decode endpoint id from TXT: '{}'", full_hex);
                return;
            }
        };

        if endpoint_id == own_id {
            return;
        }

        let addrs: Vec<SocketAddr> = peer
            .addrs()
            .iter()
            .map(|(ip, port)| SocketAddr::new(*ip, *port))
            .collect();

        if addrs.is_empty() {
            trace!("Peer {} disappeared", endpoint_id);
            return;
        }

        if !seen.lock().unwrap().insert(endpoint_id) {
            return;
        }

        trace!("Discovered peer {} at {:?}", endpoint_id, addrs);
        let _ = tx.try_send(DiscoveredPeer { endpoint_id, addrs });
    });

    let guard = builder.spawn(runtime_handle)?;
    Ok((rx, guard))
}
