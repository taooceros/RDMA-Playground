use core::panic;
use rand::random;
use rdma_sys::{ibv_qp_state::IBV_QPS_INIT, *};
use std::{
    array,
    error::Error,
    ffi::{CStr, CString},
    fmt::Display,
    io::{Read, Write},
    mem::{size_of, transmute, zeroed, MaybeUninit},
    net::{IpAddr, SocketAddr, TcpListener},
    num::NonZeroI32,
    ptr::{copy_nonoverlapping, null, null_mut},
    slice,
    thread::sleep,
    time::Duration,
};

use crate::communication_manager::CommunicationManager;

use self::{
    config::{Config, ConnectionType},
    qp_info::DestQpInfo,
};

pub mod config;
mod qp_info;

pub struct IbResource<'a> {
    ctx: *mut ibv_context,
    pd: *mut ibv_pd,
    mr: *mut ibv_mr,
    cq: *mut ibv_cq,
    qp: *mut ibv_qp,
    srq: *mut ibv_srq,
    port_attr: MaybeUninit<ibv_port_attr>,
    dev_attr: MaybeUninit<ibv_device_attr>,
    ib_buf: &'a mut [u8],
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Init,
    Connected,
    Disconnected,
    Error,
}

impl<'a> IbResource<'a> {
    pub fn new(buffer_size: usize) -> Self {
        let buffer = vec![0u8; buffer_size];

        IbResource {
            ctx: null_mut(),
            pd: null_mut(),
            mr: null_mut(),
            cq: null_mut(),
            qp: null_mut(),
            srq: null_mut(),
            port_attr: MaybeUninit::zeroed(),
            dev_attr: MaybeUninit::zeroed(),
            ib_buf: Box::leak(buffer.into_boxed_slice()),
            state: State::Init,
        }
    }

    pub fn setup_ib(&mut self, config: Config) -> Result<i32, RdmaError> {
        unsafe {
            let devices = ibv_get_device_list(null_mut());

            if devices.is_null() {
                return Err(RdmaError::GetIbDeviceError);
            }

            self.ctx = ibv_open_device(*devices);

            println!(
                "ibv_open_device: {:?}",
                CStr::from_ptr(
                    self.ctx
                        .as_ref()
                        .unwrap()
                        .device
                        .as_ref()
                        .unwrap()
                        .name
                        .as_ptr()
                )
            );

            if self.ctx.is_null() {
                panic!("Failed to open IB device");
            }

            self.pd = ibv_alloc_pd(self.ctx);

            if self.pd.is_null() {
                panic!("Failed to allocate protection domain");
            }

            let mut ret = ibv_query_port(self.ctx, 1, self.port_attr.as_mut_ptr() as *mut _);

            assert_eq!(
                self.port_attr.assume_init().state,
                ibv_port_state::IBV_PORT_ACTIVE
            );

            if ret != 0 {
                panic!("Failed to query port");
            }

            //
            /* register mr */
            /* TODO: set the buf_size twice as large as msg_size * num_concurr_msgs */
            /* the recv buffer occupies the first half while the sending buffer */
            /* occupies the second half */
            //

            // println!("ib_buf.len(): {}", self.ib_buf.len());

            self.mr = ibv_reg_mr(
                self.pd,
                self.ib_buf.as_mut_ptr() as *mut _,
                self.ib_buf.len(),
                (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_READ)
                    .0 as i32,
            );

            if self.mr.is_null() {
                panic!("Failed to register memory region");
            }

            ret = ibv_query_device(self.ctx, self.dev_attr.as_mut_ptr());

            if ret != 0 {
                panic!("Failed to query device");
            }

            // create cq

            self.cq = ibv_create_cq(self.ctx, 1, null_mut(), null_mut(), 0);

            if self.cq.is_null() {
                let os_error = std::io::Error::last_os_error();
                println!("last OS error: {os_error:?}");
                panic!("Failed to create completion queue");
            }

            // create srq

            let mut ibv_srq_init_attr: ibv_srq_init_attr = zeroed();

            ibv_srq_init_attr.attr.max_wr = self.dev_attr.assume_init().max_srq_wr as u32;
            ibv_srq_init_attr.attr.max_sge = 1;

            self.srq = ibv_create_srq(self.pd, &mut ibv_srq_init_attr);

            if self.srq.is_null() {
                let os_error = std::io::Error::last_os_error();
                println!("last OS error: {os_error:?}");
                panic!("Failed to create shared receive queue");
            }

            // create qp

            println!("Creating queue pair");

            println!("max_qp_wr: {}", self.dev_attr.assume_init().max_qp_wr);

            let mut qp_init_attr = ibv_qp_init_attr {
                qp_type: ibv_qp_type::IBV_QPT_RC,
                send_cq: self.cq,
                recv_cq: self.cq,
                srq: self.srq,
                cap: ibv_qp_cap {
                    max_send_wr: 8192,
                    max_recv_wr: 8192,
                    max_send_sge: 1,
                    max_recv_sge: 1,
                    ..zeroed()
                },
                ..zeroed()
            };

            self.qp = ibv_create_qp(self.pd, &mut qp_init_attr);

            if self.qp.is_null() {
                let os_error = std::io::Error::last_os_error();
                println!("last OS error: {os_error:?}");
                panic!("Failed to create queue pair");
            }

            self.connect_dest(config).unwrap();

            self.state = State::Connected;

            Ok(0)
        }
    }

    fn connect_qp_server(
        &mut self,
        port: u16,
        gid_index: Option<NonZeroI32>,
    ) -> Result<(), Box<dyn Error>> {
        let socket_addr = SocketAddr::new(IpAddr::V4("0.0.0.0".parse().unwrap()), port);

        let listener = TcpListener::bind(socket_addr)?;

        let (mut stream, _) = listener.accept()?;

        let buffer = &mut [0u8; size_of::<DestQpInfo>()];

        stream.read_exact(buffer)?;

        let dest_info = unsafe { *(buffer.as_ptr() as *const DestQpInfo) };

        unsafe {
            let mut gid: ibv_gid = zeroed();

            if let Some(gid_index) = gid_index {
                let ret = ibv_query_gid(self.ctx, 1, gid_index.get(), &mut gid);
                if ret > 0 {
                    panic!(
                        "Could not get local gid for gid index {}\n",
                        gid_index.get()
                    );
                }
            }

            let source_info = DestQpInfo {
                lid: self.port_attr.assume_init().lid,
                qpn: (*self.qp).qp_num,
                psn: 0,
                gid,
            };

            stream.write(transmute::<&DestQpInfo, &[u8; size_of::<DestQpInfo>()]>(
                &source_info,
            ))?;
        }

        println!("Received dest_info: {:?}", dest_info);

        self.connect_qp_to_dest(0, 1, dest_info)?;

        Ok(())
    }

    fn connect_qp_client(
        &mut self,
        server_addr: IpAddr,
        port: u16,
        gid_index: Option<NonZeroI32>,
    ) -> Result<(), Box<dyn Error>> {
        unsafe {
            let mut gid: ibv_gid = zeroed();

            if let Some(gid_index) = gid_index {
                let ret = ibv_query_gid(self.ctx, 1, gid_index.get(), &mut gid);
                if ret > 0 {
                    panic!(
                        "Could not get local gid for gid index {}\n",
                        gid_index.get()
                    );
                }
            }

            let source_info = DestQpInfo {
                lid: self.port_attr.assume_init().lid,
                qpn: (*self.qp).qp_num,
                psn: 0,
                gid: gid,
            };

            println!("{:?}", source_info);

            let socket_addr = SocketAddr::new(server_addr, port);

            let mut stream = std::net::TcpStream::connect(socket_addr).unwrap();

            let buffer = transmute::<&DestQpInfo, &[u8; size_of::<DestQpInfo>()]>(&source_info);

            stream.write_all(buffer).unwrap();

            let buffer = &mut [0u8; size_of::<DestQpInfo>()];

            stream.read_exact(buffer).unwrap();

            let dest_info = *(buffer.as_ptr() as *const DestQpInfo);

            println!("Received {:?}", dest_info);

            self.connect_qp_to_dest(0, 1, dest_info)?;
        }

        Ok(())
    }

    fn connect_dest(&mut self, config: Config) -> Result<(), Box<dyn Error>> {
        match config.connection_type {
            ConnectionType::Server { port } => self.connect_qp_server(port, config.gid_index),
            ConnectionType::Client { server_addr, port } => {
                self.connect_qp_client(server_addr, port, config.gid_index)
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
            let mut qp_attr = ibv_qp_attr {
                qp_state: IBV_QPS_INIT,
                pkey_index: 0,
                port_num: port,
                qp_access_flags: (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC
                    | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE)
                    .0,
                ..zeroed()
            };

            let ret = ibv_modify_qp(
                self.qp,
                &mut qp_attr,
                (ibv_qp_attr_mask::IBV_QP_STATE
                    | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX
                    | ibv_qp_attr_mask::IBV_QP_PORT
                    | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS)
                    .0 as i32,
            );

            if ret != 0 {
                panic!(
                    "Failed to modify QP from RESET to INIT {}",
                    std::io::Error::last_os_error()
                );
                return Err(Box::new(RdmaError::ModifyQpError {
                    from_state: "RESET",
                    to_state: "INIT",
                }));
            }

            let mut qp_attr = ibv_qp_attr {
                qp_state: ibv_qp_state::IBV_QPS_RTR,
                path_mtu: ibv_mtu::IBV_MTU_4096,
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
                qp_attr.ah_attr.is_global = 1;
                qp_attr.ah_attr.grh.dgid = dest.gid;
                qp_attr.ah_attr.grh.sgid_index = 0;
                qp_attr.ah_attr.grh.hop_limit = 1;
                qp_attr.ah_attr.grh.traffic_class = 0;
            }

            let ret = ibv_modify_qp(
                self.qp,
                &mut qp_attr,
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
                panic!(
                    "Failed to modify QP from INIT to RTR {}",
                    std::io::Error::last_os_error()
                );
                return Err(Box::new(RdmaError::ModifyQpError {
                    from_state: "INIT",
                    to_state: "RTR",
                }));
            }

            qp_attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
            qp_attr.timeout = 14;
            qp_attr.retry_cnt = 7;
            qp_attr.rnr_retry = 7;
            qp_attr.sq_psn = dest.psn;
            qp_attr.max_rd_atomic = 1;

            let ret = ibv_modify_qp(
                self.qp,
                &mut qp_attr,
                (ibv_qp_attr_mask::IBV_QP_STATE
                    | ibv_qp_attr_mask::IBV_QP_TIMEOUT
                    | ibv_qp_attr_mask::IBV_QP_RETRY_CNT
                    | ibv_qp_attr_mask::IBV_QP_RNR_RETRY
                    | ibv_qp_attr_mask::IBV_QP_SQ_PSN
                    | ibv_qp_attr_mask::IBV_QP_MAX_QP_RD_ATOMIC)
                    .0 as i32,
            );

            if ret != 0 {
                panic!("Failed to modify QP from RTR to RTS");
                return Err(Box::new(RdmaError::ModifyQpError {
                    from_state: "RTR",
                    to_state: "RTS",
                }));
            }

            self.handshake();

            Ok(())
        }
    }

    // fn send_message<M>(&mut self, message: M, wr_id: u64) {
    //     if self.state != State::Connected {
    //         panic!("QP is not connected");
    //     }

    //     let buffer = unsafe {
    //         let buffer = &mut self.ib_buf[self.buf_head..self.buf_head + size_of::<M>()];
    //         self.buf_head += size_of::<M>();
    //         buffer
    //     };
    // }

    fn handshake(&mut self) {
        const HANDSHAKE_WR_ID: u64 = 1;

        unsafe {
            self.ib_buf[0] = random();

            let ret = post_send(
                self.qp,
                self.mr.as_ref().unwrap().lkey,
                HANDSHAKE_WR_ID,
                &mut self.ib_buf[..1],
            );

            if ret != 0 {
                panic!("Failed to post send");
            }

            println!("Sent data: {}", self.ib_buf[0]);

            let ret = post_srq_recv(
                self.srq,
                self.mr.as_ref().unwrap().lkey,
                HANDSHAKE_WR_ID,
                &mut self.ib_buf[1..2],
            );

            if ret != 0 {
                panic!("Failed to post srq recv");
            }

            let mut count = 0;

            loop {
                const WC_INIT: MaybeUninit<ibv_wc> = MaybeUninit::zeroed();

                let mut wc_buffer = [WC_INIT; 16];

                let num_polled = ibv_poll_cq(self.cq, 16, &mut wc_buffer as *mut _ as *mut _);

                if num_polled < 0 {
                    panic!("Failed to poll cq");
                }

                if num_polled == 0 {
                    continue;
                }

                println!("Polled {} wc", num_polled);

                for wc in wc_buffer[..num_polled as usize].iter() {
                    let wc = wc.assume_init_ref();

                    if wc.wr_id == HANDSHAKE_WR_ID && wc.opcode == ibv_wc_opcode::IBV_WC_RECV {
                        if wc.status != ibv_wc_status::IBV_WC_SUCCESS {
                            panic!("Handshake failed");
                        }

                        println!("Receive successful with data: {}", self.ib_buf[1]);

                        count += 1;
                    }

                    if wc.wr_id == HANDSHAKE_WR_ID && wc.opcode == ibv_wc_opcode::IBV_WC_SEND {
                        if wc.status != ibv_wc_status::IBV_WC_SUCCESS {
                            panic!("Handshake failed");
                        }

                        println!("Send successful");

                        count += 1;
                    }

                    if count == 2 {
                        println!("Handshake complete");
                        break;
                    }
                }
            }
        }
    }

    pub fn send_message<M>(&mut self, message: &[M]) {
        unsafe {
            let buffer = &mut self.ib_buf[0..(size_of::<M>()) * message.len()];

            copy_nonoverlapping(message.as_ptr().cast(), buffer.as_mut_ptr(), buffer.len());

            let ret = post_send(
                self.qp,
                self.mr.as_ref().unwrap().lkey,
                2,
                &mut self.ib_buf[..size_of::<M>()],
            );

            if ret != 0 {
                panic!("Failed to post send");
            }

            // wait for work completion
            loop {
                const WC_INIT: MaybeUninit<ibv_wc> = MaybeUninit::zeroed();

                let mut wc_buffer = [WC_INIT; 1];

                let num_polled = ibv_poll_cq(self.cq, 1, wc_buffer.as_mut_ptr().cast());

                if num_polled < 0 {
                    panic!("Failed to poll cq");
                }

                if num_polled == 0 {
                    continue;
                }

                println!("Polled {} wc", num_polled);

                for wc in wc_buffer[..num_polled as usize].iter() {
                    let wc = wc.assume_init_ref();

                    if wc.wr_id == 2 && wc.opcode == ibv_wc_opcode::IBV_WC_SEND {
                        if wc.status != ibv_wc_status::IBV_WC_SUCCESS {
                            panic!("Send failed");
                        }

                        println!("Send successful");

                        return;
                    }
                }
            }
        }
    }

    pub fn recv_message<M>(&mut self) -> &[M] {
        unsafe {
            let buf_offset = self.ib_buf.len() / 2;

            let buffer = &mut self.ib_buf[buf_offset..];

            let ret = post_srq_recv(self.srq, self.mr.as_ref().unwrap().lkey, 3, buffer);

            if ret != 0 {
                panic!("Failed to post srq recv");
            }

            // wait for work completion
            loop {
                const WC_INIT: MaybeUninit<ibv_wc> = MaybeUninit::zeroed();

                let mut wc_buffer = [WC_INIT; 1];

                let num_polled = ibv_poll_cq(self.cq, 1, &mut wc_buffer as *mut _ as *mut _);

                if num_polled < 0 {
                    panic!("Failed to poll cq");
                }

                if num_polled == 0 {
                    continue;
                }

                println!("Polled {} wc", num_polled);

                for wc in wc_buffer[..num_polled as usize].iter() {
                    let wc = wc.assume_init_ref();

                    if wc.wr_id == 3 && wc.opcode == ibv_wc_opcode::IBV_WC_RECV {
                        if wc.status != ibv_wc_status::IBV_WC_SUCCESS {
                            panic!("Receive failed");
                        }

                        println!("Receive successful");

                        return transmute(buffer[..(wc.byte_len as usize)].as_ref());
                    }
                }
            }
        }
    }

    pub fn try_recv_message<M>(&mut self) -> Option<&[M]> {
        unsafe {
            let buf_offset = self.ib_buf.len() / 2;

            let buffer = &mut self.ib_buf[buf_offset..buf_offset + size_of::<M>()];

            let ret = post_srq_recv(self.srq, self.mr.as_ref().unwrap().lkey, 3, buffer);

            if ret != 0 {
                panic!("Failed to post srq recv");
            }

            // wait for work completion
            loop {
                const WC_INIT: MaybeUninit<ibv_wc> = MaybeUninit::zeroed();

                let mut wc_buffer = [WC_INIT; 1];

                let num_polled = ibv_poll_cq(self.cq, 1, wc_buffer.as_mut_ptr().cast());

                if num_polled < 0 {
                    panic!("Failed to poll cq");
                }

                if num_polled == 0 {
                    return None;
                }

                println!("Polled {} wc", num_polled);

                for wc in wc_buffer[..num_polled as usize].iter() {
                    let wc = wc.assume_init_ref();

                    if wc.wr_id == 3 && wc.opcode == ibv_wc_opcode::IBV_WC_RECV {
                        if wc.status != ibv_wc_status::IBV_WC_SUCCESS {
                            panic!("Receive failed");
                        }

                        println!("Receive successful");

                        return Some(transmute(buffer[0..(wc.byte_len as usize)].as_ref()));
                    }
                }
            }
        }
    }
}

fn post_srq_recv(srq: *mut ibv_srq, lkey: u32, wr_id: u64, buffer: &mut [u8]) -> i32 {
    unsafe {
        let mut bad_recv_wr = zeroed();

        let mut list = ibv_sge {
            addr: buffer.as_ptr() as u64,
            length: buffer.len() as u32,
            lkey: lkey,
        };

        let mut recv_wr = ibv_recv_wr {
            wr_id: wr_id,
            sg_list: &mut list,
            num_sge: 1,
            ..zeroed()
        };

        let ret = ibv_post_srq_recv(srq, &mut recv_wr, &mut bad_recv_wr);

        return ret;
    }
}

fn post_send(qp: *mut ibv_qp, lkey: u32, wr_id: u64, buffer: &mut [u8]) -> i32 {
    unsafe {
        let mut bad_send_wr = zeroed();

        let mut list = ibv_sge {
            addr: buffer.as_ptr() as u64,
            length: buffer.len() as u32,
            lkey: lkey,
        };

        let mut send_wr = ibv_send_wr {
            wr_id,
            sg_list: &mut list,
            num_sge: 1,
            opcode: ibv_wr_opcode::IBV_WR_SEND,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            ..zeroed()
        };

        let ret = ibv_post_send(qp, &mut send_wr, &mut bad_send_wr);

        return ret;
    }
}

pub struct RdmaHandShake {
    signal: u32,
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
    CreateSrqError,
    ModifyQpError {
        from_state: &'static str,
        to_state: &'static str,
    },
    RegMrError,
    CreateQpError,
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
            RdmaError::CreateQpError => write!(f, "Failed to create queue pair"),
            RdmaError::CreateSrqError => write!(f, "Failed to create shared receive queue"),
            RdmaError::ModifyQpError {
                from_state,
                to_state,
            } => write!(f, "Failed to modify QP from {} to {}", from_state, to_state),
            RdmaError::RegMrError => write!(f, "Failed to register memory region"),
        }
    }
}

impl CommunicationManager for IbResource<'_> {
    fn send_message<M>(&mut self, message: &[M]) -> Result<(), Box<dyn Error>> {
        self.send_message(message);
        Ok(())
    }

    fn try_recv_message<M>(&mut self) -> Result<Option<&[M]>, Box<dyn Error>> {
        Ok(self.try_recv_message::<M>())
    }

    fn recv_message<M>(&mut self) -> Result<&[M], Box<dyn Error>> {
        Ok(self.recv_message::<M>())
    }
}
