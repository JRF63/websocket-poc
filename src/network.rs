use std::{time::Duration, sync::{Mutex, atomic::{Ordering, AtomicU16}}};

static COUNTER: AtomicU16 = AtomicU16::new(0);
static SENT_VALUES: Mutex<[bool; (u16::MAX as usize + 1)]> = Mutex::new([false; (u16::MAX as usize + 1)]);

/// Produce a dummy task. Also mark said task for verification in the reply.
pub async fn get_tasks_from_network() -> String {
    tokio::time::sleep(Duration::from_millis(1)).await;
    let num = COUNTER.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut lock) = SENT_VALUES.lock() {
        lock[num as usize] = true;
    }
    format!("{}", num)
}

/// Checks if the value was sent before being received.
pub async fn send_replies_to_network(reply: String) {
    tokio::time::sleep(Duration::from_millis(1)).await;
    if let Ok(mut lock) = SENT_VALUES.lock() {
        let num: usize = reply.parse().unwrap();
        assert!(lock[num]);
        lock[num as usize] = false;
    }
}