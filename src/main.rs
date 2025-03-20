use crate::watcher::PortWatcher;

mod podman;
mod watcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pw = PortWatcher::new("lo");

    pw.looper(|info| {
        println!("{}", info.udp.count())
    }).await?;

    Ok(())
}

