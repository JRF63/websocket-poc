use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::{collections::HashMap, net::SocketAddr};
use tokio::{
    net::{TcpSocket, TcpStream},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tokio_tungstenite::{
    tungstenite::{self, Message},
    WebSocketStream,
};

pub struct DummyPool {
    task_txs: HashMap<SocketAddr, UnboundedSender<String>>,
    reply_tx: UnboundedSender<(SocketAddr, String)>,
}

impl DummyPool {
    pub async fn new(client_address: Vec<SocketAddr>) -> Result<DummyPool, tungstenite::Error> {
        let task_txs = HashMap::new();
        let (reply_tx, mut reply_rx) = unbounded_channel();
        let mut pool = DummyPool { task_txs, reply_tx };

        for addr in client_address {
            pool.add_node(addr).await?;
        }

        tokio::spawn(async move {
            while let Some((_addr, reply)) = reply_rx.recv().await {
                crate::network::send_replies_to_network(reply).await;
            }
        });

        Ok(pool)
    }

    async fn add_node(&mut self, addr: SocketAddr) -> Result<(), tungstenite::Error> {
        let socket = TcpSocket::new_v4()?;
        let tcp_stream = socket.connect(addr).await?;

        let (ws_stream, _response) =
            tokio_tungstenite::client_async(format!("ws://{addr}/"), tcp_stream).await?;

        let (task_tx, task_rx) = unbounded_channel();
        tokio::spawn(process(ws_stream, addr, self.reply_tx.clone(), task_rx));
        self.task_txs.insert(addr, task_tx);

        Ok(())
    }

    // Dummy random static assignment.
    fn load_balancing_algorithm(&self) -> Option<usize> {
        let n = self.task_txs.len();
        if n == 0 {
            None
        } else {
            Some(rand::thread_rng().gen_range(0..n))
        }
    }

    /// Schedules a task. Returns `true` if the task was scheduled properly otherwise returns
    /// `false` and the caller should call `schedule_task` again with the same task.
    pub async fn schedule_task(&mut self, task_json: String) -> bool {
        if let Some(idx) = self.load_balancing_algorithm() {
            let client_addr = *self
                .task_txs
                .keys()
                .nth(idx)
                .expect("Index out of bounds while querying the task transmitters");

            if let Some(sender) = self.task_txs.get(&client_addr) {
                if let Err(e) = sender.send(task_json) {
                    println!("Failed to assign task to {client_addr}: {e}");
                    let _ = self.add_node(client_addr).await;
                    return false;
                }
            }
        }
        true
    }
}

/// Handle communication with a single node.
/// 
/// Any error on the channels or the websockets would cause the function to return early thereby
/// dropping `task_rx` and causing the `UnboundedSender::send` in `DummyPool::schedule_task` to
/// fail. This would in turn cause `DummyPool` to create a new websocket with the same address as
/// the one that failed.
async fn process(
    ws_stream: WebSocketStream<TcpStream>,
    addr: SocketAddr,
    reply_tx: UnboundedSender<(SocketAddr, String)>,
    mut task_rx: UnboundedReceiver<String>,
) -> Result<(), tungstenite::Error> {
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
