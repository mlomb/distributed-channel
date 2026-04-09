use anyhow::Result;
use log::{info, trace};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use swarm_discovery::Discoverer;
use tokio::sync::mpsc;

/// Start mDNS discovery, broadcasting this node's presence and discovering others.
///
/// `peer_id` is a unique string identifying this node (used as the mDNS peer name).
/// If `broadcast_port` is `Some(port)`, the node advertises itself on the LAN.
/// If `None`, the node only discovers others.
///
/// Returns a channel receiver yielding discovered `SocketAddr`s and a drop guard.
pub fn start_discovery(
    service_name: String,
    peer_id: String,
    broadcast_port: Option<u16>,
    runtime_handle: &tokio::runtime::Handle,
) -> Result<(mpsc::Receiver<SocketAddr>, swarm_discovery::DropGuard)> {
    let (tx, rx) = mpsc::channel::<SocketAddr>(64);

    let own_id = peer_id.clone();
    let mut builder = Discoverer::new_interactive(service_name, peer_id);

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
    }

    let seen = Arc::new(Mutex::new(HashSet::<String>::new()));
    builder = builder.with_callback(move |peer_id_str, peer| {
        if peer_id_str == own_id {
            return;
        }

        let addrs: Vec<SocketAddr> = peer
            .addrs()
            .iter()
            .map(|(ip, port)| SocketAddr::new(*ip, *port))
            .collect();

        if addrs.is_empty() {
            return;
        }

        if !seen.lock().unwrap().insert(peer_id_str.to_string()) {
            return;
        }

        trace!("Discovered peer {} at {:?}", peer_id_str, addrs);
        for addr in addrs {
            let _ = tx.try_send(addr);
        }
    });

    let guard = builder.spawn(runtime_handle)?;
    Ok((rx, guard))
}
