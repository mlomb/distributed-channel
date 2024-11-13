use distributed_channel::node::NodeSetup;
use futures::StreamExt;
use log::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

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

    if std::env::args().nth(1).unwrap() == "consumer" {
        consumer();
    } else {
        producer();
    }
}

fn consumer() {
    let (_node, mut tx) = NodeSetup::default().into_consumer::<Init, WorkDefinition, WorkResult>();

    let mut i = 0;
    while let Some(work) = tx.blocking_recv() {
        println!("PROCESSING WORK: {:?}", work.work_definition);
        println!("i = {}", i);
        i += 1;

        std::thread::sleep(std::time::Duration::from_secs(1));

        work.sender
            .send(WorkResult {
                id: work.work_definition.id,
            })
            .unwrap();

        if i == 5 {
            break;
        }
    }
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
