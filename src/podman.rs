use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyperlocal::{UnixClientExt, UnixConnector, Uri};

pub struct Podman {
    path: String,
    client: Client<UnixConnector, Full<Bytes>>,
}


impl Podman {

    pub fn new(unix_socket: Option<String>) -> Self {
        let path = unix_socket.unwrap_or_else(|| {
            let uid = unsafe { libc::geteuid() };
            format!("/run/user/{}/podman/podman.sock", uid)
        });

        Self {
            path,
            client: Client::unix(),
        }
    }

    pub async fn get(&mut self, path: &str, body: Option<&[u8]>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let url = Uri::new(self.path.as_str(), path).into();

        let mut response = self.client.get(url).await?;

        let mut buffer: Vec<u8> = Vec::new();

        while let Some(frame_result) = response.frame().await {
            let frame = frame_result?;

            if let Some(segment) = frame.data_ref() {
                for v in segment.iter() {
                    buffer.push(*v);
                }
            }
        }

        Ok(buffer)
    }


}


