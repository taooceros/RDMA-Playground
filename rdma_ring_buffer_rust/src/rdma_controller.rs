use rand::random;
use rdma_sys::*;
use std::{
    error::Error,
    fmt::Display,
    io::{Read, Write},
    mem::{size_of, transmute, zeroed, MaybeUninit},
    net::{IpAddr, SocketAddr, TcpListener},
    ptr::{null, null_mut},
};

use self::{
    config::{Config, ConnectionType},
    qp_info::DestQpInfo,
};

pub mod config;
mod qp_info;

pub struct IbResource<'a, const BUFFER_SIZE: usize = 8192> {
    ctx: *mut ibv_context,
    pd: *mut ibv_pd,
    mr: *mut ibv_mr,
    cq: *mut ibv_cq,
    qp: *mut ibv_qp,
    srq: *mut ibv_srq,
    port_attr: MaybeUninit<ibv_port_attr>,
    dev_attr: MaybeUninit<ibv_device_attr>,
    ib_buf: &'a mut [u8; BUFFER_SIZE],
}

impl<'a, const BUFFER_SIZE: usize> IbResource<'a, BUFFER_SIZE> {
    pub fn new() -> Self {
        IbResource {
            ctx: null_mut(),
            pd: null_mut(),
            mr: null_mut(),
            cq: null_mut(),
            qp: null_mut(),
            srq: null_mut(),
            port_attr: MaybeUninit::uninit(),
            dev_attr: MaybeUninit::uninit(),
            ib_buf: Box::leak(Box::new([0; BUFFER_SIZE])),
        }
    }

    pub fn setup_ib(&mut self, config: Config) -> Result<i32, RdmaError> {
        unsafe {
            let devices = ibv_get_device_list(null_mut());

            if devices.is_null() {
                return Err(RdmaError::GetIbDeviceError);
            }

            self.ctx = ibv_open_device(*devices);

            if self.ctx.is_null() {
                return Err(RdmaError::OpenIbDeviceError);
            }

            self.pd = ibv_alloc_pd(self.ctx);

            if self.pd.is_null() {
                return Err(RdmaError::AllocPdError);
            }

            let mut ret = ibv_query_port(self.ctx, 1, self.port_attr.as_mut_ptr() as *mut _);

            if ret != 0 {
                return Err(RdmaError::QueryPortError(ret));
            }

            //
            /* register mr */
            /* TODO: set the buf_size twice as large as msg_size * num_concurr_msgs */
            /* the recv buffer occupies the first half while the sending buffer */
            /* occupies the second half */
            //

            self.mr = ibv_reg_mr(
                self.pd,
                self.ib_buf.as_mut_ptr() as *mut _,
                BUFFER_SIZE,
                (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_READ)
                    .0 as i32,
            );

            if self.mr.is_null() {
                return Err(RdmaError::RegMrError);
            }

            ret = ibv_query_device(self.ctx, self.dev_attr.as_mut_ptr());

            if ret != 0 {
                return Err(RdmaError::QueryDeviceError(ret));
            }

            // create cq

            self.cq = ibv_create_cq(
                self.ctx,
                self.dev_attr.assume_init().max_cqe,
                null_mut(),
                null_mut(),
                0,
            );

            if self.cq.is_null() {
                return Err(RdmaError::CreateCqError);
            }

            // create srq

            let mut ibv_srq_init_attr = MaybeUninit::<ibv_srq_init_attr>::uninit();

            ibv_srq_init_attr.assume_init().attr.max_wr =
                self.dev_attr.assume_init().max_srq_wr as u32;
            ibv_srq_init_attr.assume_init().attr.max_sge = 1;

            self.srq = ibv_create_srq(self.pd, ibv_srq_init_attr.as_mut_ptr());

            if self.srq.is_null() {
                return Err(RdmaError::CreateCqError);
            }

            // create qp

            let mut qp_init_attr = ibv_qp_init_attr {
                send_cq: self.cq,
                recv_cq: self.cq,
                srq: self.srq,
                cap: ibv_qp_cap {
                    max_send_wr: self.dev_attr.assume_init().max_qp_wr as u32,
                    max_recv_wr: self.dev_attr.assume_init().max_qp_wr as u32,
                    max_send_sge: 1,
                    max_recv_sge: 1,
                    max_inline_data: 0,
                },
                qp_type: ibv_qp_type::IBV_QPT_RC,
                qp_context: null_mut(),
                sq_sig_all: 0,
            };

            self.qp = ibv_create_qp(self.pd, &mut qp_init_attr);

            if self.qp.is_null() {
                return Err(RdmaError::CreateCqError);
            }

            Ok(0)
        }
    }

    fn connect_qp_server(&mut self, port: u16) -> Result<(), Box<dyn Error>> {
        let socket_addr = SocketAddr::new(IpAddr::V4("0.0.0.0".parse().unwrap()), port);

        let listener = TcpListener::bind(socket_addr)?;

        let (mut stream, _) = listener.accept()?;

        let buffer = &mut [0u8; size_of::<DestQpInfo>()];

        stream.read_exact(buffer)?;

        let dest_info = unsafe { *(buffer.as_ptr() as *const DestQpInfo) };

        self.connect_qp_to_dest(1, 1, dest_info)?;

        Ok(())
    }

    fn connect_qp_client(&mut self, server_addr: IpAddr, port: u16) -> Result<(), Box<dyn Error>> {
        let socket_addr = SocketAddr::new(server_addr, port);

        let mut stream = std::net::TcpStream::connect(socket_addr)?;

        unsafe {
            let dest_info = DestQpInfo {
                lid: self.port_attr.assume_init().lid,
                qpn: (*self.qp).qp_num,
                psn: random::<u32>() & 0xffffff,
                gid: zeroed(),
            };

            let buffer = transmute::<&DestQpInfo, &[u8; size_of::<DestQpInfo>()]>(&dest_info);

            stream.write_all(buffer)?;
        }

        Ok(())
    }

    fn connect_dest(&mut self, config: Config) -> Result<(), Box<dyn Error>> {
        match config.connection_type {
            ConnectionType::Server { port } => self.connect_qp_server(port),
            ConnectionType::Client { server_addr, port } => {
                self.connect_qp_client(server_addr, port)
            }
        }
    }

    fn connect_qp_to_dest(
        &mut self,
        sl: u8,
        port: u8,
        dest: DestQpInfo,
    ) -> Result<(), Box<dyn Error>> {
        unsafe {
            let mut attr = ibv_qp_attr {
                qp_state: ibv_qp_state::IBV_QPS_RTR,
                path_mtu: ibv_mtu::IBV_MTU_1024,
                dest_qp_num: dest.qpn,
                rq_psn: dest.psn,
                max_dest_rd_atomic: 1,
                min_rnr_timer: 12,
                ah_attr: ibv_ah_attr {
                    is_global: 0,
                    dlid: dest.lid,
                    sl,
                    src_path_bits: 0,
                    port_num: port,
                    ..zeroed()
                },
                ..zeroed()
            };

            if dest.gid.global.interface_id != 0 {
                attr.ah_attr.is_global = 1;
                attr.ah_attr.grh.dgid = dest.gid;
                attr.ah_attr.grh.sgid_index = 0;
                attr.ah_attr.grh.hop_limit = 1;
                attr.ah_attr.grh.traffic_class = 0;
            }

            let ret = ibv_modify_qp(
                self.qp,
                &mut attr,
                (ibv_qp_attr_mask::IBV_QP_STATE
                    | ibv_qp_attr_mask::IBV_QP_AV
                    | ibv_qp_attr_mask::IBV_QP_PATH_MTU
                    | ibv_qp_attr_mask::IBV_QP_DEST_QPN
                    | ibv_qp_attr_mask::IBV_QP_RQ_PSN
                    | ibv_qp_attr_mask::IBV_QP_MAX_DEST_RD_ATOMIC
                    | ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER)
                    .0 as i32,
            );

            if ret != 0 {
                return Err(Box::new(RdmaError::ModifyQpError {
                    from_state: "IBV_QPS_INIT",
                    to_state: "IBV_QPS_RTR",
                }));
            }

            attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
            attr.timeout = 14;
            attr.retry_cnt = 7;
            attr.rnr_retry = 7;
            attr.sq_psn = 0;
            attr.max_rd_atomic = 1;

            let ret = ibv_modify_qp(
                self.qp,
                &mut attr,
                (ibv_qp_attr_mask::IBV_QP_STATE
                    | ibv_qp_attr_mask::IBV_QP_TIMEOUT
                    | ibv_qp_attr_mask::IBV_QP_RETRY_CNT
                    | ibv_qp_attr_mask::IBV_QP_RNR_RETRY
                    | ibv_qp_attr_mask::IBV_QP_SQ_PSN
                    | ibv_qp_attr_mask::IBV_QP_MAX_QP_RD_ATOMIC)
                    .0 as i32,
            );

            if ret != 0 {
                return Err(Box::new(RdmaError::ModifyQpError {
                    from_state: "RTR",
                    to_state: "RTS",
                }));
            }

            Ok(())
        }
    }
}

pub enum ConnectionError {
    TcpListenerError,
    TcpStreamError,
}

#[derive(Debug)]
pub enum RdmaError {
    GetIbDeviceError,
    OpenIbDeviceError,
    AllocPdError,
    QueryPortError(i32),
    QueryDeviceError(i32),
    CreateCqError,
    ModifyQpError {
        from_state: &'static str,
        to_state: &'static str,
    },
    RegMrError,
}
// const IBV_ACCESS_RELAXED_ORDERING: i32 = IBV_ACCESS_OPTIONAL_FIRST;

impl Error for RdmaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

impl Display for RdmaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RdmaError::GetIbDeviceError => write!(f, "Failed to get IB device list"),
            RdmaError::OpenIbDeviceError => write!(f, "Failed to open IB device"),
            RdmaError::AllocPdError => write!(f, "Failed to allocate protection domain"),
            RdmaError::QueryPortError(ret) => {
                write!(f, "Failed to query port with return value {}", ret)
            }
            RdmaError::QueryDeviceError(ret) => {
                write!(f, "Failed to query device with return value {}", ret)
            }
            RdmaError::CreateCqError => write!(f, "Failed to create completion queue"),
            RdmaError::ModifyQpError {
                from_state,
                to_state,
            } => write!(f, "Failed to modify QP from {} to {}", from_state, to_state),
            RdmaError::RegMrError => write!(f, "Failed to register memory region"),
        }
    }
}
