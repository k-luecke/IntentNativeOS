// kernel/src/net/socket.rs - Versioned Kernel Socket Subsystem & Ring 3 ABI

use super::{Ipv4Address, NetError};
use super::tcp::{TcpSocketState, TcpState, TCP_FLAG_SYN, TCP_FLAG_ACK, TCP_FLAG_PSH, TCP_FLAG_FIN};

pub const AF_INET: u64 = 2;
pub const SOCK_STREAM: u64 = 1; // TCP
pub const SOCK_DGRAM: u64 = 2;  // UDP

pub const MAX_SOCKETS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Unused,
    Udp,
    Tcp,
}

pub struct KernelSocket {
    pub sock_id: u64,
    pub owner_pid: u64,
    pub sock_type: SocketType,
    pub local_ip: Ipv4Address,
    pub local_port: u16,
    pub remote_ip: Ipv4Address,
    pub remote_port: u16,
    pub tcp_state: Option<TcpSocketState>,
    pub udp_rx_queue: [u8; 2048],
    pub udp_rx_len: usize,
    pub is_connected: bool,
    pub cap_token: u64,
    pub is_revoked: bool,
}

impl KernelSocket {
    pub const fn empty() -> Self {
        Self {
            sock_id: 0,
            owner_pid: 0,
            sock_type: SocketType::Unused,
            local_ip: Ipv4Address::UNASSIGNED,
            local_port: 0,
            remote_ip: Ipv4Address::UNASSIGNED,
            remote_port: 0,
            tcp_state: None,
            udp_rx_queue: [0u8; 2048],
            udp_rx_len: 0,
            is_connected: false,
            cap_token: 0,
            is_revoked: false,
        }
    }
}

pub struct SocketTable {
    pub sockets: [KernelSocket; MAX_SOCKETS],
    next_sock_id: u64,
    next_ephemeral_port: u16,
}

impl SocketTable {
    pub const fn new() -> Self {
        Self {
            sockets: [
                KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(),
                KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(),
                KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(),
                KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(), KernelSocket::empty(),
            ],
            next_sock_id: 1,
            next_ephemeral_port: 49152,
        }
    }

    pub fn allocate_socket(&mut self, owner_pid: u64, cap_token: u64, sock_type: SocketType, local_ip: Ipv4Address) -> Result<u64, NetError> {
        for slot in self.sockets.iter_mut() {
            if slot.sock_type == SocketType::Unused {
                let sock_id = self.next_sock_id;
                self.next_sock_id += 1;

                let local_port = self.next_ephemeral_port;
                self.next_ephemeral_port = if self.next_ephemeral_port >= 65500 { 49152 } else { self.next_ephemeral_port + 1 };

                slot.sock_id = sock_id;
                slot.owner_pid = owner_pid;
                slot.sock_type = sock_type;
                slot.local_ip = local_ip;
                slot.local_port = local_port;
                slot.remote_ip = Ipv4Address::UNASSIGNED;
                slot.remote_port = 0;
                slot.udp_rx_len = 0;
                slot.is_connected = false;
                slot.cap_token = cap_token;
                slot.is_revoked = false;

                if sock_type == SocketType::Tcp {
                    slot.tcp_state = Some(TcpSocketState::new(local_ip, local_port));
                } else {
                    slot.tcp_state = None;
                }

                return Ok(sock_id);
            }
        }
        Err(NetError::QueueFull)
    }

    pub fn revoke_sockets_for_capability(&mut self, owner_pid: u64, cap_token: u64) -> usize {
        let mut count = 0;
        for slot in self.sockets.iter_mut() {
            if slot.sock_type != SocketType::Unused && slot.owner_pid == owner_pid && (cap_token == 0 || slot.cap_token == cap_token) {
                slot.is_revoked = true;
                slot.is_connected = false;
                if let Some(ref mut tcp) = slot.tcp_state {
                    tcp.state = super::tcp::TcpState::Closed;
                }
                count += 1;
            }
        }
        count
    }

    pub fn get_socket_mut(&mut self, sock_id: u64, caller_pid: u64) -> Result<&mut KernelSocket, NetError> {
        for slot in self.sockets.iter_mut() {
            if slot.sock_id == sock_id && slot.sock_type != SocketType::Unused {
                if slot.owner_pid != caller_pid && caller_pid != 0 {
                    return Err(NetError::ConnectionRefused);
                }
                if slot.is_revoked {
                    return Err(NetError::ConnectionRefused);
                }
                return Ok(slot);
            }
        }
        Err(NetError::ConnectionRefused)
    }

    pub fn find_udp_socket_mut(&mut self, dst_port: u16) -> Option<&mut KernelSocket> {
        for slot in self.sockets.iter_mut() {
            if slot.sock_type == SocketType::Udp && slot.local_port == dst_port {
                return Some(slot);
            }
        }
        None
    }

    pub fn find_tcp_socket_mut(&mut self, local_port: u16, remote_port: u16) -> Option<&mut KernelSocket> {
        for slot in self.sockets.iter_mut() {
            if slot.sock_type == SocketType::Tcp && slot.local_port == local_port && slot.remote_port == remote_port {
                return Some(slot);
            }
        }
        None
    }

    pub fn close_socket(&mut self, sock_id: u64, caller_pid: u64) -> Result<(), NetError> {
        for slot in self.sockets.iter_mut() {
            if slot.sock_id == sock_id {
                if slot.owner_pid != caller_pid && caller_pid != 0 {
                    return Err(NetError::ConnectionRefused);
                }
                *slot = KernelSocket::empty();
                return Ok(());
            }
        }
        Err(NetError::ConnectionRefused)
    }
}

pub static mut GLOBAL_SOCKET_TABLE: SocketTable = SocketTable::new();
