//! Standalone consumer that discovers producers on the LAN via mDNS.
//!
//! Usage: cargo run --example consumer

use distributed_channel::{start_consumer_node, NodeSetup};
use log::LevelFilter;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work { pub id: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Result { pub id: u32 }

fn process(input: Work) -> Result {
    println!("Processing work {}", input.id);
    std::thread::sleep(std::time::Duration::from_millis(10));
    Result { id: 2 * input.id }
}

fn main() {
    env_logger::Builder::new()
        .filter_module("distributed_channel", LevelFilter::Info)
        .init();

    println!("Consumer running. Discovering producers via mDNS...");
    start_consumer_node(NodeSetup::default(), 8, process);
}
