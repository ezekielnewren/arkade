use crate::watcher::PortWatcher;

mod podman;
mod watcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pw = PortWatcher::new("lo");

    pw.looper(|info| {
        let mut tcp = Vec::new();
        for i in 0..std::cmp::min(info.tcp.size(), 0x10000) {
            if info.tcp.test(i) {
                tcp.push(i.to_string());
            }
        }
        let tcp_ports = tcp.join(", ");

        let mut udp = Vec::new();
        for i in 0..std::cmp::min(info.udp.size(), 0x10000) {
            if info.udp.test(i) {
                udp.push(i.to_string());
            }
        }
        let udp_ports = udp.join(", ");

        println!("tcp: {}", tcp_ports);
        println!("udp: {}", udp_ports);
    }).await?;

    Ok(())
}

