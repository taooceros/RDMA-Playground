use std::fmt::Debug;
use std::{io, mem::zeroed};

use rdma_sys::*;
use zerocopy::{AsBytes, FromBytes};

use super::{memory_region::MemoryRegion, IbResource};

impl IbResource {
    // Safety: data must be part of the memory region
    pub unsafe fn post_send(
        &mut self,
        wr_id: u64,
        mr: &mut MemoryRegion,
        data: &[(impl FromBytes + AsBytes)],
        signal: bool,
    ) -> io::Result<()> {
        unsafe {
            let mut bad_send_wr = zeroed();

            let lkey = mr.mr.as_ref().unwrap().lkey;

            let u8_ref = data.as_bytes();

            let mut list = ibv_sge {
                addr: u8_ref.as_ptr() as u64,
                length: u8_ref.len().try_into().unwrap(),
                lkey,
            };

            let send_flags = if signal {
                ibv_send_flags::IBV_SEND_SIGNALED.0
            } else {
                0
            };

            let mut send_wr = ibv_send_wr {
                wr_id,
                sg_list: &mut list,
                num_sge: 1,
                opcode: ibv_wr_opcode::IBV_WR_SEND,
                send_flags,
                ..zeroed()
            };

            let errno = ibv_post_send(self.qp, &mut send_wr, &mut bad_send_wr);

            if errno != 0 {
                return Err(io::Error::last_os_error());
            }

            return Ok(());
        }
    }
}

pub struct SendFlagBuilder {
    flags: u32,
}

impl SendFlagBuilder {
    pub fn new() -> Self {
        Self { flags: 0 }
    }

    pub fn signaled(mut self) -> Self {
        self.flags |= ibv_send_flags::IBV_SEND_SIGNALED.0;
        self
    }

    pub fn fenced(mut self) -> Self {
        self.flags |= ibv_send_flags::IBV_SEND_FENCE.0;
        self
    }

    pub fn inline(mut self) -> Self {
        self.flags |= ibv_send_flags::IBV_SEND_INLINE.0;
        self
    }

    pub fn solicited(mut self) -> Self {
        self.flags |= ibv_send_flags::IBV_SEND_SOLICITED.0;
        self
    }

    pub fn build(self) -> u32 {
        self.flags
    }
}
