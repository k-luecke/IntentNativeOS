// userspace/sdk/src/net.rs - Typed Network Socket API for Ring 3 Applications

use super::syscall::{raw_syscall, SyscallError, SYS_NET_SOCKET, SYS_NET_CONNECT, SYS_NET_SEND, SYS_NET_RECV, SYS_NET_CLOSE, SYS_NET_POLL, SYS_NET_RESOLVE};

pub const AF_INET: u64 = 2;
pub const SOCK_STREAM: u64 = 1; // TCP
pub const SOCK_DGRAM: u64 = 2;  // UDP

pub struct UdpSocket {
    fd: u64,
}

impl UdpSocket {
    pub fn bind(cap_token: u64) -> Result<Self, SyscallError> {
        let full_token = crate::resolve_full_token(cap_token);
        let token_low = full_token as u64;
        let token_high = (full_token >> 64) as u64;
        unsafe {
            let fd = raw_syscall(SYS_NET_SOCKET, AF_INET, SOCK_DGRAM, token_low, token_low, token_high)?;
            Ok(Self { fd })
        }
    }

    pub fn connect(&self, ip: [u8; 4], port: u16) -> Result<(), SyscallError> {
        let ip_u32 = u32::from_be_bytes(ip) as u64;
        unsafe {
            raw_syscall(SYS_NET_CONNECT, self.fd, ip_u32, port as u64, 0, 0)?;
            Ok(())
        }
    }

    pub fn send(&self, data: &[u8]) -> Result<usize, SyscallError> {
        unsafe {
            let sent = raw_syscall(SYS_NET_SEND, self.fd, data.as_ptr() as u64, data.len() as u64, 0, 0)?;
            Ok(sent as usize)
        }
    }

    pub fn recv(&self, buf: &mut [u8]) -> Result<usize, SyscallError> {
        unsafe {
            let received = raw_syscall(SYS_NET_RECV, self.fd, buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0)?;
            Ok(received as usize)
        }
    }

    pub fn poll(&self) -> Result<u64, SyscallError> {
        unsafe {
            raw_syscall(SYS_NET_POLL, self.fd, 0, 0, 0, 0)
        }
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        unsafe {
            let _ = raw_syscall(SYS_NET_CLOSE, self.fd, 0, 0, 0, 0);
        }
    }
}

pub struct TcpStream {
    fd: u64,
}

impl TcpStream {
    pub fn connect(ip: [u8; 4], port: u16, cap_token: u64) -> Result<Self, SyscallError> {
        let full_token = crate::resolve_full_token(cap_token);
        let token_low = full_token as u64;
        let token_high = (full_token >> 64) as u64;
        unsafe {
            let fd = raw_syscall(SYS_NET_SOCKET, AF_INET, SOCK_STREAM, token_low, token_low, token_high)?;
            let ip_u32 = u32::from_be_bytes(ip) as u64;
            raw_syscall(SYS_NET_CONNECT, fd, ip_u32, port as u64, 0, 0)?;
            Ok(Self { fd })
        }
    }

    pub fn send(&self, data: &[u8]) -> Result<usize, SyscallError> {
        unsafe {
            let sent = raw_syscall(SYS_NET_SEND, self.fd, data.as_ptr() as u64, data.len() as u64, 0, 0)?;
            Ok(sent as usize)
        }
    }

    pub fn recv(&self, buf: &mut [u8]) -> Result<usize, SyscallError> {
        unsafe {
            let received = raw_syscall(SYS_NET_RECV, self.fd, buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0)?;
            Ok(received as usize)
        }
    }

    pub fn poll(&self) -> Result<u64, SyscallError> {
        unsafe {
            raw_syscall(SYS_NET_POLL, self.fd, 0, 0, 0, 0)
        }
    }

    pub fn close(self) -> Result<(), SyscallError> {
        let fd = self.fd;
        core::mem::forget(self);
        unsafe {
            raw_syscall(SYS_NET_CLOSE, fd, 0, 0, 0, 0)?;
        }
        Ok(())
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        unsafe {
            let _ = raw_syscall(SYS_NET_CLOSE, self.fd, 0, 0, 0, 0);
        }
    }
}

/// Resolves a domain hostname to an IPv4 address using authorized capability token
pub fn resolve_host(hostname: &str, cap_token: u64) -> Result<[u8; 4], SyscallError> {
    let full_token = crate::resolve_full_token(cap_token);
    let token_low = full_token as u64;
    let token_high = (full_token >> 64) as u64;
    unsafe {
        let ip_u32 = raw_syscall(
            SYS_NET_RESOLVE,
            hostname.as_ptr() as u64,
            hostname.len() as u64,
            token_low,
            token_low,
            token_high,
        )?;
        Ok((ip_u32 as u32).to_be_bytes())
    }
}
