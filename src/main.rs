use std::fmt::{Debug, Display, Formatter};
use regex::Regex;
use crate::watcher::*;

mod podman;
mod watcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pw = PortWatcher::new("lo");

    pw.watch_udp(34197);
    pw.watch_udp(34198);

    pw.watch_tcp(25565);
    pw.watch_tcp(25566);

    println!("PortWatcher listening...");

    pw.looper(|event| {
        match event {
            PortEvent::TCP(port) => println!("tcp: {}", port),
            PortEvent::UDP(port) => println!("udp: {}", port),
        }
    }).await?;

    Ok(())
}





#[cfg(test)]
mod tests {
    use regex::Regex;
    use crate::parse_ports;
    use crate::watcher::PortEvent;

    #[test]
    fn test_port_parsing() {
        let expected = vec![PortEvent::TCP(25565), PortEvent::UDP(34197)];
        let t: Vec<String> = expected.iter().map(|v| v.to_string()).collect();
        let input = t.join(",");
        let actual = parse_ports(input.as_str()).unwrap();

        assert_eq!(expected, actual);
    }

}

