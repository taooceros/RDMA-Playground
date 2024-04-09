use std::error::Error;

pub trait CommunicationManager {
    fn send_message<M>(&mut self, message: &[M]) -> Result<(), Box<dyn Error>>;
    fn recv_message<M>(&mut self) -> Result<&[M], Box<dyn Error>>;
    fn try_recv_message<M>(&mut self) -> Result<Option<&[M]>, Box<dyn Error>>;
}
