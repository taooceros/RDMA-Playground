use derivative::Derivative;
use rdma_sys::ibv_gid;

#[derive(Derivative)]
#[derivative(Debug, Clone, Copy)]
pub struct DestQpInfo {
    pub lid: u16,
    pub qpn: u32,
    pub psn: u32,
    #[derivative(Debug = "ignore")]
    pub gid: ibv_gid,
}
