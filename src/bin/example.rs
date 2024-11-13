use distributed_channel::{node::NodeSetup, worker::Worker};
use log::*;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Init {
    pub id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDefinition {
    pub id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResult {
    pub id: u32,
}

fn main() {
    env_logger::Builder::new()
        .filter_module("distributed_channel", LevelFilter::Trace)
        .init();

    let node_setup = NodeSetup::default();

    if std::env::args().nth(1).unwrap() == "consumer" {
        // consumer();
        let worker = Worker::new(node_setup, 8, process);

        std::thread::park();
    } else {
        producer();
    }
}

fn process(init: Arc<Mutex<Init>>, input: WorkDefinition) -> WorkResult {
    std::thread::sleep(std::time::Duration::from_millis(10));

    WorkResult { id: 100 * input.id }
}

fn producer() {
    let init = Init { id: 123 };

    let (_node, tx, rx) =
        NodeSetup::default().into_producer::<Init, WorkDefinition, WorkResult>(init);

    let mut id = 0;
    loop {
        crossbeam_channel::select! {
            recv(rx) -> res => {
                let res = res.unwrap();
                println!("PRODUCER Received: {:?}", res);
            },
            send(tx, WorkDefinition { id }) -> res => {
                println!("PRODUCER Sent: {:?}", id);
                id += 1;
            },
        }
    }
}
