use crate::SERVER_PORT;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::{net::Ipv4Addr, time::Duration};
use tokio::net::TcpSocket;
use tokio_tungstenite::tungstenite::{self, Message};

struct DummyNode;

impl DummyNode {
    fn new() -> DummyNode {
        DummyNode
    }

    async fn process_tasks_loop(&self) -> Result<(), tungstenite::Error> {
        // Handles disconnection by recreating the WebSocket with each call
        let (mut tx, mut rx) = {
            let socket = TcpSocket::new_v4()?;
            let tcp_stream = socket
                .connect((Ipv4Addr::LOCALHOST, SERVER_PORT).into())
                .await?;
            let (ws_stream, _response) = tokio_tungstenite::client_async(
                format!("ws://localhost:{SERVER_PORT}/"),
                tcp_stream,
            )
            .await?;

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
        // 10% chance of failure
        rand::thread_rng().gen_range(0..100) < 10
    }
}

pub async fn client_loop_with_retry() {
    let node = DummyNode::new();
    loop {
        if let Err(e) = node.process_tasks_loop().await {
            // Maybe log error then retry
            println!("{e}");
        }
    }
}
