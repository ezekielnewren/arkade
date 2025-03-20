use std::ffi::CString;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};
use bitset::BitSet;
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::io::unix::AsyncFd;
use tokio::time::timeout;

pub struct PortWatcher {
    interface: String,
    tcp: BitSet,
    udp: BitSet,
}

pub struct PortInfo {
    timestamp: Instant,
    report: bool,
    pub tcp: BitSet,
    pub udp: BitSet,
}

impl PortWatcher {

    pub fn new(iface: &str) -> Self {
        Self {
            interface: iface.to_string(),
            tcp: BitSet::with_capacity(0x10000),
            udp: BitSet::with_capacity(0x10000),
        }
    }

    pub(crate) async fn looper<F>(&mut self, mut lambda: F) -> Result<(), Box<dyn std::error::Error>>
    where F: FnMut(&PortInfo)
    {
        // let interface = "lo"; // Change to match your network interface (e.g., ens3)
        let iface_name = CString::new(self.interface.as_str()).expect("CString conversion failed");

        // Create a raw socket (AF_PACKET for Ethernet-level capture)
        let socket = Socket::new(Domain::PACKET, Type::RAW, Some(Protocol::from(libc::ETH_P_ALL as i32)))?;
        // Set socket to non-blocking mode
        socket.set_nonblocking(true)?;

        // Get interface index
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
                std::mem::size_of::<libc::sockaddr_ll>() as u32,
            )
        };

        if bind_result != 0 {
            panic!("Failed to bind socket to interface {}: {}", self.interface, std::io::Error::last_os_error());
        }

        // Wrap in Tokio AsyncFd
        let async_fd = AsyncFd::new(socket)?;
        let mut buf: [MaybeUninit<u8>; 65536] = unsafe { MaybeUninit::uninit().assume_init() };

        let mut callback = |info: &mut PortInfo| {
            if info.timestamp.elapsed().as_secs_f32() >= 1.0 {
                info.timestamp = Instant::now();
                if info.report {
                    lambda(&info);
                }
                info.report = false;
                info.tcp.reset();
                info.udp.reset();
            }
        };

        let mut info = PortInfo {
            timestamp: Instant::now(),
            report: false,
            tcp: BitSet::with_capacity(0x10000),
            udp: BitSet::with_capacity(0x10000),
        };


        loop {
            match timeout(Duration::from_secs(1), async_fd.readable()).await {
                Ok(Ok(mut guard)) => {
                    match guard.try_io(|fd| fd.get_ref().recv_from(&mut buf)) {
                        Ok(Ok((len, _addr))) => {
                            let vec: Vec::<u8> = buf[..len].iter().map(|v| unsafe { v.assume_init() }).collect();
                            parse_packet(vec.as_slice(), &mut info);
                            callback(&mut info);
                        },
                        _ => {}
                    }
                }
                Ok(Err(e)) => eprintln!("Socket error: {}", e), // Handle socket errors
                Err(_) => {
                    callback(&mut info);
                }
            }

        }
    }
}


/// Parses an Ethernet frame and filters only TCP & UDP packets
fn parse_packet(packet: &[u8], info: &mut PortInfo) -> Option<()> {
    let ethernet = EthernetPacket::new(packet)?;

    match ethernet.get_ethertype() {
        pnet::packet::ethernet::EtherTypes::Ipv4 => {
            let ipv4 = Ipv4Packet::new(ethernet.payload())?;
            match ipv4.get_next_level_protocol() {
                IpNextHeaderProtocols::Tcp => {
                    let tcp = TcpPacket::new(ipv4.payload())?;
                    info.tcp.set(tcp.get_destination() as usize, true);
                    info.report = true;
                }
                IpNextHeaderProtocols::Udp => {
                    let udp = UdpPacket::new(ipv4.payload())?;
                    info.udp.set(udp.get_destination() as usize, true);
                    info.report = true;
                }
                _ => return None, // Ignore ICMP (Ping), IGMP, and other protocols
            }
        }
        pnet::packet::ethernet::EtherTypes::Ipv6 => {
            let ipv6 = Ipv6Packet::new(ethernet.payload())?;
            match ipv6.get_next_header() {
                IpNextHeaderProtocols::Tcp => {
                    let tcp = TcpPacket::new(ipv6.payload())?;
                    info.tcp.set(tcp.get_destination() as usize, true);
                    info.report = true;
                }
                IpNextHeaderProtocols::Udp => {
                    let udp = UdpPacket::new(ipv6.payload())?;
                    info.udp.set(udp.get_destination() as usize, true);
                    info.report = true;
                }
                _ => return None,
            }
        }
        _ => return None,
    }

    Some(())
}

