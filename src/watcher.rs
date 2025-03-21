use std::ffi::CString;
use std::fmt::{Debug, Display, Formatter};
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};
use bitvec::prelude::*;
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use regex::Regex;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::io::unix::AsyncFd;
use tokio::time::timeout;

#[derive(PartialEq, Eq)]
pub enum PortInfo {
    TCP(u16),
    UDP(u16),
}

impl Display for PortInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PortInfo::TCP(port) => write!(f, "{}/tcp", *port),
            PortInfo::UDP(port) => write!(f, "{}/udp", *port),
        }
    }
}

impl Debug for PortInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PortInfo::TCP(port) => write!(f, "{}/tcp", *port),
            PortInfo::UDP(port) => write!(f, "{}/udp", *port),
        }
    }
}

pub struct ParsePortError {
    message: String,
}

impl Debug for ParsePortError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl Display for ParsePortError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl std::error::Error for ParsePortError {}


pub fn parse_ports(list: &str) -> Result<Vec<PortInfo>, Box<dyn std::error::Error>> {
    let re = Regex::new(r"^(?i)(\d+)/(tcp|udp)$").unwrap();
    let mut actual: Vec<PortInfo> = Vec::new();

    for v in list.split(',') {
        if let Some(cap) = re.captures(v) {
            let port = u16::from_str_radix(&cap[1], 10)?;
            let protocol = cap[2].to_lowercase();

            match protocol.as_str() {
                "tcp" => actual.push(PortInfo::TCP(port)),
                "udp" => actual.push(PortInfo::UDP(port)),
                _ => return Err(Box::new(ParsePortError {
                    message: format!("invalid protocol: {}", protocol),
                })),
            }
        }

    }

    Ok(actual)
}


pub struct PortSet {
    tcp: BitVec,
    udp: BitVec,
}

impl Default for PortSet {
    fn default() -> Self {
        Self {
            tcp: BitVec::repeat(false, 0x10000),
            udp: BitVec::repeat(false, 0x10000),
        }
    }
}

impl PortSet {

    pub fn reset(&mut self) {
        self.tcp.fill(false);
        self.udp.fill(false);
    }

}

pub struct PortWatcher {
    interface: String,
    watch: PortSet,
    reported: PortSet,
    timestamp: Instant,
    aggregate_window: f32,
}



impl PortWatcher {

    pub fn new(iface: String) -> Self {
        Self {
            interface: iface,
            watch: PortSet::default(),
            reported: PortSet::default(),
            timestamp: Instant::now(),
            aggregate_window: 5.0,
        }
    }

    pub fn watch(&mut self, port: PortInfo) {
        match port {
            PortInfo::TCP(port) => self.watch.tcp.set(port as usize, true),
            PortInfo::UDP(port) => self.watch.udp.set(port as usize, true),
        }
    }

    pub fn unwatch(&mut self, port: PortInfo) {
        match port {
            PortInfo::TCP(port) => self.watch.tcp.set(port as usize, false),
            PortInfo::UDP(port) => self.watch.udp.set(port as usize, false),
        }
    }

    fn process_packet(&mut self, packet: &[u8]) -> Option<PortInfo> {
        let ethernet = EthernetPacket::new(packet)?;

        match ethernet.get_ethertype() {
            pnet::packet::ethernet::EtherTypes::Ipv4 => {
                let ipv4 = Ipv4Packet::new(ethernet.payload())?;
                match ipv4.get_next_level_protocol() {
                    IpNextHeaderProtocols::Tcp => {
                        let tcp: u16 = TcpPacket::new(ipv4.payload())?.get_destination();
                        if self.watch.tcp[tcp as usize] && !self.reported.tcp[tcp as usize] {
                            self.reported.tcp.set(tcp as usize, true);
                            return Some(PortInfo::TCP(tcp));
                        }
                    }
                    IpNextHeaderProtocols::Udp => {
                        let udp: u16 = UdpPacket::new(ipv4.payload())?.get_destination();
                        if self.watch.udp[udp as usize] && !self.reported.udp[udp as usize] {
                            self.reported.udp.set(udp as usize, true);
                            return Some(PortInfo::UDP(udp));
                        }
                    }
                    _ => return None, // Ignore ICMP (Ping), IGMP, and other protocols
                }
            }
            pnet::packet::ethernet::EtherTypes::Ipv6 => {
                let ipv6 = Ipv6Packet::new(ethernet.payload())?;
                match ipv6.get_next_header() {
                    IpNextHeaderProtocols::Tcp => {
                        let tcp: u16 = TcpPacket::new(ipv6.payload())?.get_destination();
                        if self.watch.tcp[tcp as usize] && !self.reported.tcp[tcp as usize] {
                            self.reported.tcp.set(tcp as usize, true);
                            return Some(PortInfo::TCP(tcp));
                        }
                    }
                    IpNextHeaderProtocols::Udp => {
                        let udp: u16 = UdpPacket::new(ipv6.payload())?.get_destination();
                        if self.watch.udp[udp as usize] && !self.reported.udp[udp as usize] {
                            self.reported.udp.set(udp as usize, true);
                            return Some(PortInfo::UDP(udp));
                        }
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }

        None
    }

    pub(crate) async fn looper<F>(&mut self, mut lambda: F) -> Result<(), Box<dyn std::error::Error>>
    where F: FnMut(PortInfo)
    {
        let iface_name = CString::new(self.interface.as_str())?;
        let socket = Socket::new(Domain::PACKET, Type::RAW, Some(Protocol::from(libc::ETH_P_ALL as i32)))?;
        socket.set_nonblocking(true)?;

        let if_index = unsafe { libc::if_nametoindex(iface_name.as_ptr() as *const i8) };
        if if_index == 0 {
            panic!("Failed to get index for interface {}", self.interface);
        }

        // Bind to the specific interface
        let sockaddr_ll = libc::sockaddr_ll {
            sll_family: libc::AF_PACKET as u16,
            sll_protocol: (libc::ETH_P_ALL as u16).to_be(), // Convert to big-endian
            sll_ifindex: if_index as i32,
            sll_hatype: 0,
            sll_pkttype: 0,
            sll_halen: 0,
            sll_addr: [0; 8], // Hardware address (not needed here)
        };

        // Bind the socket to the interface
        let bind_result = unsafe {
            libc::bind(
                socket.as_raw_fd(),
                &sockaddr_ll as *const _ as *const libc::sockaddr,
                size_of::<libc::sockaddr_ll>() as u32,
            )
        };

        if bind_result != 0 {
            panic!("Failed to bind socket to interface {}: {}", self.interface, std::io::Error::last_os_error());
        }

        let async_fd = AsyncFd::new(socket)?;
        let mut buf: [MaybeUninit<u8>; 65536] = unsafe { MaybeUninit::uninit().assume_init() };

        loop {
            match timeout(Duration::from_secs_f32(self.aggregate_window), async_fd.readable()).await {
                Ok(Ok(mut guard)) => {
                    match guard.try_io(|fd| fd.get_ref().recv_from(&mut buf)) {
                        Ok(Ok((len, _addr))) => {
                            let vec: Vec::<u8> = buf[..len].iter().map(|v| unsafe { v.assume_init() }).collect();
                            if let Some(event) = self.process_packet(vec.as_slice()) {
                                lambda(event);
                            }
                            if self.timestamp.elapsed().as_secs_f32() > self.aggregate_window {
                                self.timestamp = Instant::now();
                                self.reported.reset();
                            }
                        },
                        _ => {}
                    }
                }
                Ok(Err(e)) => eprintln!("Socket error: {}", e), // Handle socket errors
                Err(_) => {
                    self.timestamp = Instant::now();
                    self.reported.reset();
                }
            }
        }
    }
}




