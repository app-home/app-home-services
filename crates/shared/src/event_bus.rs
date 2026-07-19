use tokio::sync::broadcast;

use crate::domain::events::Event;

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> (Self, broadcast::Receiver<Event>) {
        let (tx, rx) = broadcast::channel(capacity);
        (Self { tx }, rx)
    }

    pub fn publish(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}
