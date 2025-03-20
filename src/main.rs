use crate::watcher::{PortEvent, PortWatcher};

mod podman;
mod watcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pw = PortWatcher::new("lo");

    pw.watch_udp(34197);
    pw.watch_udp(34198);

    pw.watch_tcp(25565);
    pw.watch_tcp(25566);

    pw.looper(|event| {
        match event {
            PortEvent::TCP(port) => println!("tcp: {}", port),
            PortEvent::UDP(port) => println!("udp: {}", port),
        }
    }).await?;

    Ok(())
}

