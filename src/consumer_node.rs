use crate::{Networked, NodeSetup};
use log::info;
use std::sync::{Arc, Mutex};

pub fn start_consumer_node<I, W, R>(
    setup: NodeSetup,
    mut num_threads: usize,
    func: fn(Arc<Mutex<I>>, W) -> R,
)
// TODO: -> Result<!, Error> when ! stabilizes
where
    I: Networked,
    W: Networked,
    R: Networked,
{
    let (_node, work_rx) = setup.into_consumer::<I, W, R>();

    if num_threads == 0 {
        num_threads = std::thread::available_parallelism()
            .expect("not to fail")
            .into();

        info!("Using {} threads", num_threads);
    }

    for _ in 0..num_threads {
        let work_rx = work_rx.clone();

        std::thread::spawn(move || loop {
            match work_rx.recv_blocking() {
                Ok(work) => {
                    work.sender
                        .send(func(work.peer_data, work.work_definition))
                        .unwrap();
                }
                Err(_) => unreachable!("should not be dropped"),
            }
        });
    }

    std::thread::park();
}
