use crate::{Networked, NodeSetup};

pub fn start_consumer_node<W, R>(
    setup: NodeSetup,
    num_threads: usize,
    func: fn(W) -> R,
) where
    W: Networked + Sync,
    R: Networked + Sync,
{
    let (_node, work_rx) = setup.into_consumer::<W, R>();

    for _ in 0..num_threads {
        let work_rx = work_rx.clone();
        std::thread::spawn(move || loop {
            match work_rx.recv_blocking() {
                Ok(entry) => { let _ = entry.sender.send(func(entry.work)); }
                Err(_) => break,
            }
        });
    }

    std::thread::park();
}
