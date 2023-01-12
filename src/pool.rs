use crate::SERVER_PORT;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    sync::Mutex,
};
use tokio_tungstenite::tungstenite::{self, Message};

pub struct DummyPool {
    task_txs: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<String>>>>,
}

impl DummyPool {
    pub async fn new() -> Result<DummyPool, tungstenite::Error> {
        let task_txs = Arc::new(Mutex::new(HashMap::new()));
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, SERVER_PORT)).await?;
        let (reply_tx, mut reply_rx) = unbounded_channel();

        let hash_map = task_txs.clone();
        tokio::spawn(async move {
            loop {
                let (tcp_stream, addr) = listener.accept().await?;
                let (task_tx, task_rx) = unbounded_channel();
                hash_map.lock().await.insert(addr, task_tx);
                tokio::spawn(process(tcp_stream, addr, reply_tx.clone(), task_rx));
            }
            #[allow(unreachable_code)]
            Result::<(), tungstenite::Error>::Ok(())
        });

        tokio::spawn(async move {
            while let Some((_addr, reply)) = reply_rx.recv().await {
                crate::network::send_replies_to_network(reply).await;
            }
        });

        Ok(DummyPool { task_txs })
    }

    // Dummy random static assignment.
    async fn load_balancing_algorithm(&self) -> Option<usize> {
        let n = self.task_txs.lock().await.len();
        if n == 0 {
            None
        } else {
            Some(rand::thread_rng().gen_range(0..n))
        }
    }

    pub async fn schedule_task(&mut self, task_json: String) -> Option<()> {
        if let Some(idx) = self.load_balancing_algorithm().await {
            let mut task_txs = self.task_txs.lock().await;
            let client_addr = *task_txs
                .keys()
                .nth(idx)
                .expect("Index out of bounds while querying the task transmitters");

            if let Some(sender) = task_txs.get(&client_addr) {
                if let Err(e) = sender.send(task_json) {
                    println!("Failed to assign task to {client_addr}: {e}");
                    task_txs.remove(&client_addr);
                    return None;
                }
            }

            Some(())
        } else {
            None
        }
    }
}

async fn process(
    tcp_stream: TcpStream,
    addr: SocketAddr,
    reply_tx: UnboundedSender<(SocketAddr, String)>,
    mut task_rx: UnboundedReceiver<String>,
) -> Result<(), tungstenite::Error> {
    let ws_stream = tokio_tungstenite::accept_async(tcp_stream).await?;
    let (mut tx, mut rx) = ws_stream.split();

    // Handles sending tasks
    let sender = tokio::spawn(async move {
        while let Some(task_json) = task_rx.recv().await {
            let message = Message::text(&task_json);
            tx.send(message).await?;
        }
        Result::<Status, tungstenite::Error>::Ok(Status::SchedulerChannelClosed)
    });

    // Handles replies from the node
    let receiver = tokio::spawn(async move {
        while let Some(msg) = rx.next().await {
            let msg = msg?;
            let reply = msg.into_text()?;
            if reply_tx.send((addr, reply)).is_err() {
                return Ok(Status::SchedulerChannelClosed);
            }
        }
        // Client no longer active
        Result::<Status, tungstenite::Error>::Ok(Status::ClientStoppedTransmitting)
    });

    // WebSocket endpoints `tx` and `rx` are tied together - i.e, if one has an error the other
    // one should too - so we only await one Tokio task
    let _ = receiver.await;
    sender.abort();

    Ok(())
}

enum Status {
    SchedulerChannelClosed,
    ClientStoppedTransmitting,
}
