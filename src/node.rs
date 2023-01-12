use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::{net::SocketAddr, time::Duration};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::{self, Message};

struct DummyNode {
    listener: TcpListener,
}

impl DummyNode {
    async fn new(addr: SocketAddr) -> Result<DummyNode, tungstenite::Error> {
        let listener = TcpListener::bind(addr).await?;
        Ok(DummyNode { listener })
    }

    async fn process_tasks_loop(&self) -> Result<(), tungstenite::Error> {
        // Handles disconnection by recreating the WebSocket with each call
        let (mut tx, mut rx) = {
            let (tcp_stream, _addr) = self.listener.accept().await?;
            let ws_stream = tokio_tungstenite::accept_async(tcp_stream).await?;
            ws_stream.split()
        };

        loop {
            match rx.next().await {
                Some(msg) => {
                    let msg = msg?;
                    let task = msg.to_text()?;
                    let solution = self.process_single_task(task).await?;
                    let reply = Message::text(solution);
                    tx.send(reply).await?;
                }
                None => return Ok(()), // Server stopped sending tasks
            }
        }
    }

    async fn process_single_task(&self, task_json: &str) -> Result<String, tungstenite::Error> {
        if self.simulated_network_failure() {
            println!(
                "Simulated network failure at {}",
                self.listener
                    .local_addr()
                    .expect("Failed to get address of listener")
            );
            return Err(tungstenite::Error::ConnectionClosed);
        }

        // Use serde_json::from_str(task_json) to unpack the task

        let millis = rand::thread_rng().gen_range(10..1000);

        // Simulate compute intensive work
        tokio::time::sleep(Duration::from_millis(millis)).await;

        // Echo the task
        Ok(task_json.to_owned())
    }

    fn simulated_network_failure(&self) -> bool {
        // 5% chance of failure
        rand::thread_rng().gen_range(0..100) < 5
    }
}

pub async fn client_loop_with_retry(addr: SocketAddr) {
    let node = DummyNode::new(addr)
        .await
        .expect("Unable to create `DummyNode`");
    loop {
        if let Err(_e) = node.process_tasks_loop().await {
            // Maybe log error
        }
    }
}
