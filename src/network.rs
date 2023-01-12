use std::{time::Duration, sync::atomic::{Ordering, AtomicU64}};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub async fn get_tasks_from_network() -> String {
    tokio::time::sleep(Duration::from_millis(1)).await;
    format!("{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

pub async fn send_replies_to_network(reply: String) {
    tokio::time::sleep(Duration::from_millis(1)).await;
    println!("Got reply: {reply}");
}