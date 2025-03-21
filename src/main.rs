use std::fmt::{Debug, Display, Formatter};
use chrono::Local;
use regex::Regex;
use crate::watcher::*;

use clap::Parser;

mod podman;
mod watcher;

#[derive(Parser, Debug)]
#[command(name = "arkade")]
#[command(about = "Game server port watcher", long_about = None)]
struct Args {
    #[arg(long = "interface")]
    interface: Option<String>,

    /// Ports to watch, like "25565/tcp,34197/udp"
    #[arg(long = "port-watcher")]
    port_watcher: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args = Args::parse();

    if let Some(spec) = args.port_watcher {
        let ports = parse_ports(&spec)?;

        let mut pw = PortWatcher::new(args.interface.unwrap());

        for port in ports {
            pw.watch(port);
        }

        println!("PortWatcher listening...");
        pw.looper(|event| {
            let now = Local::now();
            println!("{}", now.format("%Y-%m-%d %H:%M:%S"));
            println!("{}", event);
        }).await?;
    }

    Ok(())
}





#[cfg(test)]
mod tests {
    use regex::Regex;
    use crate::parse_ports;
    use crate::watcher::PortInfo;

    #[test]
    fn test_port_parsing() {
        let expected = vec![PortInfo::TCP(25565), PortInfo::UDP(34197)];
        let t: Vec<String> = expected.iter().map(|v| v.to_string()).collect();
        let input = t.join(",");
        let actual = parse_ports(input.as_str()).unwrap();

        assert_eq!(expected, actual);
    }

}

