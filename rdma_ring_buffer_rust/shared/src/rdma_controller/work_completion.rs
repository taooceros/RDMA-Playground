use std::fmt::Debug;

use rdma_sys::ibv_wc;

pub struct WorkCompletion(ibv_wc);

impl Debug for WorkCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkCompletion")
            .field("wr_id", &self.0.wr_id)
            .field("status", &self.0.status)
            .field("opcode", &self.0.opcode)
            .field("byte_len", &self.0.byte_len)
            .finish()
    }
}

impl From<ibv_wc> for WorkCompletion {
    fn from(wc: ibv_wc) -> Self {
        WorkCompletion(wc)
    }
}