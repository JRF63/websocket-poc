mod network;
mod node;
mod pool;

use node::client_loop_with_retry;
use pool::DummyPool;
use std::net::{Ipv4Addr, SocketAddr};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), std::io::Error> {
    let num_clients = 10;
    let client_addresses: Vec<SocketAddr> = (0..num_clients)
        .map(|i| (Ipv4Addr::LOCALHOST, 9000 + i).into())
        .collect();

    for &addr in &client_addresses {
        tokio::spawn(client_loop_with_retry(addr));
    }

    let handle = tokio::spawn(async move {
        let mut server = DummyPool::new(client_addresses)
            .await
            .expect("Unable to create `DummyServer`");
        loop {
            let task = network::get_tasks_from_network().await;
            // Reschedule task until it succeeds
            while !server.schedule_task(task.clone()).await {}
        }
    });

    let _ = handle.await;

    Ok(())
}
