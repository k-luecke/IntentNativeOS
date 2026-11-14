// kernel/src/net/tcp.rs - Transmission Control Protocol (RFC 793, RFC 7323)

use super::{Ipv4Address, NetError};
use super::ipv4::compute_checksum;

pub const TCP_HEADER_MIN_LEN: usize = 20;

// TCP Flags
pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;
pub const TCP_FLAG_PSH: u8 = 0x08;
pub const TCP_FLAG_ACK: u8 = 0x10;
pub const TCP_FLAG_URG: u8 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

#[derive(Debug, Clone, Copy)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8,
    pub flags: u8,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

impl TcpHeader {
    pub fn parse(payload: &[u8]) -> Result<(Self, &[u8]), NetError> {
        if payload.len() < TCP_HEADER_MIN_LEN {
            return Err(NetError::BufferTooShort);
        }

        let src_port = u16::from_be_bytes([payload[0], payload[1]]);
        let dst_port = u16::from_be_bytes([payload[2], payload[3]]);
        let seq_num = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        let ack_num = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
        let data_offset = payload[12] >> 4;
        let flags = payload[13];
        let window_size = u16::from_be_bytes([payload[14], payload[15]]);
        let checksum = u16::from_be_bytes([payload[16], payload[17]]);
        let urgent_ptr = u16::from_be_bytes([payload[18], payload[19]]);

        let header_len = (data_offset as usize) * 4;
        if payload.len() < header_len {
            return Err(NetError::BufferTooShort);
        }

        let data = &payload[header_len..];

        Ok((
            Self {
                src_port,
                dst_port,
                seq_num,
                ack_num,
                data_offset,
                flags,
                window_size,
                checksum,
                urgent_ptr,
            },
            data,
        ))
    }

    pub fn write_to(
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        seq_num: u32,
        ack_num: u32,
        flags: u8,
        window_size: u16,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, NetError> {
        let total_len = TCP_HEADER_MIN_LEN + data.len();
        if out.len() < total_len {
            return Err(NetError::BufferTooShort);
        }

        out[0..2].copy_from_slice(&src_port.to_be_bytes());
        out[2..4].copy_from_slice(&dst_port.to_be_bytes());
        out[4..8].copy_from_slice(&seq_num.to_be_bytes());
        out[8..12].copy_from_slice(&ack_num.to_be_bytes());
        out[12] = 0x50; // Data offset 5 (20 bytes)
        out[13] = flags;
        out[14..16].copy_from_slice(&window_size.to_be_bytes());
        out[16..18].copy_from_slice(&[0, 0]); // Zero checksum
        out[18..20].copy_from_slice(&0u16.to_be_bytes()); // Urgent pointer
        out[TCP_HEADER_MIN_LEN..total_len].copy_from_slice(data);

        // Pseudo-header checksum
        let mut pseudo_buf = [0u8; 1536];
        if 12 + total_len <= pseudo_buf.len() {
            pseudo_buf[0..4].copy_from_slice(&src_ip.0);
            pseudo_buf[4..8].copy_from_slice(&dst_ip.0);
            pseudo_buf[8] = 0;
            pseudo_buf[9] = 6; // Protocol TCP
            pseudo_buf[10..12].copy_from_slice(&(total_len as u16).to_be_bytes());
            pseudo_buf[12..12 + total_len].copy_from_slice(&out[..total_len]);

            let csum = compute_checksum(&pseudo_buf[..12 + total_len]);
            let final_csum = if csum == 0 { 0xFFFF } else { csum };
            out[16..18].copy_from_slice(&final_csum.to_be_bytes());
        }

        Ok(total_len)
    }
}

pub struct TcpSocketState {
    pub local_ip: Ipv4Address,
    pub remote_ip: Ipv4Address,
    pub local_port: u16,
    pub remote_port: u16,
    pub state: TcpState,
    pub local_seq: u32,
    pub remote_seq: u32,
    pub rx_buffer: [u8; 4096],
    pub rx_head: usize,
    pub rx_tail: usize,
    pub rx_count: usize,
    pub unacked_seq: u32,
    pub unacked_len: usize,
    pub unacked_data: [u8; 1500],
    pub rto_counter: u32,
    pub retransmit_attempts: u32,
    pub fin_sent: bool,
    pub fin_ack_received: bool,
}

impl TcpSocketState {
    pub const fn new(local_ip: Ipv4Address, local_port: u16) -> Self {
        Self {
            local_ip,
            remote_ip: Ipv4Address::UNASSIGNED,
            local_port,
            remote_port: 0,
            state: TcpState::Closed,
            local_seq: 0x1000_0000,
            remote_seq: 0,
            rx_buffer: [0u8; 4096],
            rx_head: 0,
            rx_tail: 0,
            rx_count: 0,
            unacked_seq: 0,
            unacked_len: 0,
            unacked_data: [0u8; 1500],
            rto_counter: 0,
            retransmit_attempts: 0,
            fin_sent: false,
            fin_ack_received: false,
        }
    }

    pub fn record_sent_segment(&mut self, seq: u32, data: &[u8]) {
        self.unacked_seq = seq;
        let copy_len = core::cmp::min(data.len(), self.unacked_data.len());
        self.unacked_data[..copy_len].copy_from_slice(&data[..copy_len]);
        self.unacked_len = copy_len;
        self.rto_counter = 100;
        self.retransmit_attempts = 0;
    }

    pub fn acknowledge_seq(&mut self, ack: u32) {
        if self.unacked_len > 0 {
            let expected_ack = self.unacked_seq.wrapping_add(self.unacked_len as u32);
            if ack >= expected_ack {
                self.unacked_len = 0;
                self.rto_counter = 0;
                self.retransmit_attempts = 0;
            }
        }
    }

    pub fn tick_rto(&mut self) -> bool {
        if self.unacked_len > 0 && self.retransmit_attempts < 5 {
            if self.rto_counter > 0 {
                self.rto_counter -= 1;
            } else {
                self.retransmit_attempts += 1;
                self.rto_counter = 100 * (1 << self.retransmit_attempts);
                return true;
            }
        }
        false
    }

    pub fn push_rx_data(&mut self, data: &[u8]) -> usize {
        let mut written = 0;
        for &b in data {
            if self.rx_count < self.rx_buffer.len() {
                self.rx_buffer[self.rx_tail] = b;
                self.rx_tail = (self.rx_tail + 1) % self.rx_buffer.len();
                self.rx_count += 1;
                written += 1;
            } else {
                break;
            }
        }
        written
    }

    pub fn read_rx_data(&mut self, dest: &mut [u8]) -> usize {
        let mut read_bytes = 0;
        while self.rx_count > 0 && read_bytes < dest.len() {
            dest[read_bytes] = self.rx_buffer[self.rx_head];
            self.rx_head = (self.rx_head + 1) % self.rx_buffer.len();
            self.rx_count -= 1;
            read_bytes += 1;
        }
        read_bytes
    }
}
