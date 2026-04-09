//! Runs a producer and a consumer in the same process for quick testing.
//!
//! Usage: cargo run --example local

use distributed_channel::NodeSetup;
use log::LevelFilter;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Result {
    pub id: u32,
    pub doubled: u32,
}

fn process(work: Work) -> Result {
    std::thread::sleep(std::time::Duration::from_millis(50));
    Result { id: work.id, doubled: work.id * 2 }
}

fn main() {
    env_logger::Builder::new()
        .filter_module("distributed_channel", LevelFilter::Info)
        .init();

    // Start producer
    let (_producer, work_tx, result_rx) =
        NodeSetup::default().into_producer::<Work, Result>();

    // Start consumer (discovers producer via mDNS on the same machine)
    let (_consumer, work_rx) =
        NodeSetup::default().into_consumer::<Work, Result>();

    // Worker threads
    for _ in 0..4 {
        let work_rx = work_rx.clone();
        std::thread::spawn(move || loop {
            match work_rx.recv_blocking() {
                Ok(entry) => { let _ = entry.sender.send(process(entry.work)); }
                Err(_) => break,
            }
        });
    }

    // Print results
    let result_rx2 = result_rx.clone();
    std::thread::spawn(move || loop {
        match result_rx2.recv_blocking() {
            Ok(r) => println!("Result: work {} => {}", r.id, r.doubled),
            Err(_) => break,
        }
    });

    // Send work
    for id in 0..20 {
        work_tx.send_blocking(Work { id }).expect("send work");
        println!("Sent work {}", id);
    }

    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("Done!");
}
