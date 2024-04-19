use std::io;
use std::ops::DerefMut;

use std::ops::Deref;

use std::mem::MaybeUninit;

use bytemuck::NoUninit;
use rdma_sys::{ibv_access_flags, ibv_mr, ibv_reg_mr};

use super::IbResource;

pub struct MemoryRegion {
    pub(crate) mr: *mut ibv_mr,
}

unsafe impl Send for MemoryRegion {}

unsafe impl Sync for MemoryRegion {}

impl Drop for MemoryRegion {
    fn drop(&mut self) {
        unsafe {
            rdma_sys::ibv_dereg_mr(self.mr);
        }
    }
}

impl Deref for MemoryRegion {
    type Target = ibv_mr;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mr }
    }
}

impl DerefMut for MemoryRegion {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mr }
    }
}

impl IbResource {
    pub fn register_memory_region<'a, T: Sized + Copy>(
        &mut self,
        buffer: &'a mut [T],
    ) -> io::Result<MemoryRegion> {
        unsafe {
            eprintln!(
                "Registering memory region address: {:p} with length: {}",
                buffer.as_ptr(),
                buffer.len()
            );

            let mr = ibv_reg_mr(
                self.pd,
                buffer.as_ptr().cast_mut().cast(),
                buffer.len() * std::mem::size_of::<T>(),
                (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_READ)
                    .0 as i32,
            );

            if mr.is_null() {
                return Err(io::Error::last_os_error());
            }

            Ok(MemoryRegion { mr })
        }
    }
}
