use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use tokio::io::{self, AsyncWriteExt as _};


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uid = unsafe { libc::geteuid() };
    let podman_socket = format!("/run/user/{}/podman/podman.sock", uid);
    println!("Podman socket path: {:?}", podman_socket);

    let url = Uri::new(podman_socket, "/v4.0.0/libpod/_ping").into();

    let client: Client<UnixConnector, Full<Bytes>> = Client::unix();

    let mut response = client.get(url).await?;

    while let Some(frame_result) = response.frame().await {
        let frame = frame_result?;

        if let Some(segment) = frame.data_ref() {
            io::stdout().write_all(segment.iter().as_slice()).await?;
        }
    }

    Ok(())

}
