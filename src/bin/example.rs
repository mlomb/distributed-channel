use crossbeam_channel::select;
use distributed_channel::{
    consumer::{ConsumerPeer, WorkEntry},
    producer::ProducerPeer,
};
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

const CHANNEL_SIZE: usize = 1;

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_module("distributed_channel", LevelFilter::Trace)
        .init();

    let init = Init { id: 123 };

    if std::env::args().nth(1).unwrap() == "consumer" {
        let (s, r) = crossbeam_channel::bounded(CHANNEL_SIZE);

        tokio::spawn(async move {
            ConsumerPeer::<Init, WorkDefinition, WorkResult>::new(s)
                .run()
                .await;
        });

        while let Ok(work) = r.recv() {
            println!("PROCESSING WORK: {:?}", work.work_definition);

            std::thread::sleep(std::time::Duration::from_secs(1));

            work.sender
                .send(WorkResult {
                    id: work.work_definition.id,
                })
                .unwrap();
        }
    } else {
        let (s, r) = crossbeam_channel::bounded::<WorkDefinition>(CHANNEL_SIZE);
        let (u, v) = crossbeam_channel::bounded::<WorkResult>(1);

        std::thread::spawn(move || ProducerPeer::new(init, r, u).run());

        let mut id = 0;
        loop {
            select! {
                recv(v) -> res => {
                    let res = res.unwrap();
                    println!("PRODUCER Received: {:?}", res);
                },
                send(s, WorkDefinition { id }) -> res => {
                    let res = res.unwrap();
                    println!("PRODUCER Sent: {:?}", res);
                    id += 1;
                },
            }
        }
    }
}
