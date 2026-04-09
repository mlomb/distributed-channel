//! Standalone producer that distributes work to consumers on the LAN.
//!
//! Usage: cargo run --example producer

use distributed_channel::NodeSetup;
use log::LevelFilter;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work { pub id: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Result { pub id: u32 }

fn main() {
    env_logger::Builder::new()
        .filter_module("distributed_channel", LevelFilter::Info)
        .init();

    let (_node, work_tx, result_rx) =
        NodeSetup::default().into_producer::<Work, Result>();

    println!("Producer running. Waiting for consumers via mDNS...");

    // Print results
    let result_rx2 = result_rx.clone();
    std::thread::spawn(move || loop {
        match result_rx2.recv_blocking() {
            Ok(r) => println!("Received result: {:?}", r),
            Err(_) => break,
        }
    });

    // Send work continuously
    let mut id = 0u32;
    loop {
        work_tx.send_blocking(Work { id }).expect("send work");
        println!("Sent work {}", id);
        id += 1;
    }
}
