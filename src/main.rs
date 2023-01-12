mod node;
mod network;
mod pool;

use node::client_loop_with_retry;
use pool::DummyPool;

const SERVER_PORT: u16 = 10000;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), std::io::Error> {
    let num_clients = 1;
    
    for _ in 0..num_clients {
        tokio::spawn(client_loop_with_retry());
    }

    let handle = tokio::spawn(async move {
        let mut server = DummyPool::new().await.expect("Unable to create `DummyServer`");
        loop {
            let task = network::get_tasks_from_network().await;
            server.schedule_task(task).await;
        }
    });
    
    let _ = handle.await;

    Ok(())
}
