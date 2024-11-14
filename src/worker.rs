use crate::{consumer::WorkRx, Networked, Node, NodeSetup};
use std::{
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

pub struct Worker<I, W, R> {
    node: Node,
    work_rx: WorkRx<I, W, R>,

    threads: Vec<JoinHandle<()>>,

    func: fn(Arc<Mutex<I>>, W) -> R,
}

impl<I, W, R> Worker<I, W, R>
where
    I: Networked,
    W: Networked,
    R: Networked,
{
    pub fn new(setup: NodeSetup, num_threads: usize, func: fn(Arc<Mutex<I>>, W) -> R) -> Self {
        let (node, work_rx) = setup.into_consumer::<I, W, R>();
        let mut threads = Vec::new();

        for _ in 0..num_threads {
            let work_rx = work_rx.clone();

            let thread = std::thread::spawn(move || loop {
                match work_rx.recv_blocking() {
                    Ok(work) => {
                        work.sender
                            .send(func(work.peer_data, work.work_definition))
                            .unwrap();
                    }
                    Err(_) => unreachable!("should not be dropped"),
                }
            });

            threads.push(thread);
        }

        Worker {
            node,
            work_rx,
            threads,
            func,
        }
    }

    // TODO: resize number of threads
}
