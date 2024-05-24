use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub port: u16,
    pub message_size: usize,
    pub batch_size: usize,
    pub buffer_size: usize,
    pub duration: Duration,
}
