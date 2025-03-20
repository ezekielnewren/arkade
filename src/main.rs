mod podman;

use std::time::Instant;
use bitset::BitSet;
use http_body_util::{BodyExt};
use hyperlocal::{UnixClientExt};
use pnet::datalink;
use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use tokio::io::{AsyncWriteExt as _};
use crate::podman::Podman;

fn main() {
    let iface_name = "lo"; // Change to match your network interface (e.g., ens3)

    let interface = datalink::interfaces()
        .into_iter()
        .find(|iface| iface.name == iface_name)
        .expect("Could not find network interface");

    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!("Failed to create channel: {}", e),
    };

    println!("Listening for TCP/UDP packets on {}", iface_name);

    let mut info = PortInfo {
        timestamp: Instant::now(),
        tcp: BitSet::default(),
        udp: BitSet::default(),
    };

    loop {
        match rx.next() {
            Ok(packet) => {
                // Ensure we only process TCP & UDP packets
                if parse_packet(packet, &mut info).is_some() {
                    if info.timestamp.elapsed().as_secs_f32() >= 1.0 {
                        info.timestamp = Instant::now();
                        println!("captured packets");
                    }
                }
            }
            Err(e) => eprintln!("Failed to read packet: {}", e),
        }
    }
}

struct PortInfo {
    timestamp: Instant,
    tcp: BitSet,
    udp: BitSet,
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
                }
                IpNextHeaderProtocols::Udp => {
                    let udp = UdpPacket::new(ipv4.payload())?;
                    info.udp.set(udp.get_destination() as usize, true);
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
                }
                IpNextHeaderProtocols::Udp => {
                    let udp = UdpPacket::new(ipv6.payload())?;
                    info.udp.set(udp.get_destination() as usize, true);
                }
                _ => return None,
            }
        }
        _ => return None,
    }

    Some(())
}

