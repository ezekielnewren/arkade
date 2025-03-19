use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request};
use hyper::header::CONTENT_TYPE;
use hyper_util::client::legacy::Client;
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use serde_json::json;

pub struct Podman {
    path: String,
    client: Client<UnixConnector, Full<Bytes>>,
    api_version: String,
}


impl Podman {

    pub async fn new(unix_socket: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = unix_socket.unwrap_or_else(|| {
            let uid = unsafe { libc::geteuid() };
            format!("/run/user/{}/podman/podman.sock", uid)
        });

        let mut it = Self {
            path,
            client: Client::unix(),
            api_version: String::default(),
        };

        let result = it.get("/version", Vec::new()).await?;
        let info: serde_json::Value = serde_json::from_slice(result.as_slice())?;
        let version = info["Components"]
            .as_array().unwrap()
            .iter()
            .find(|comp| comp["Name"] == "Podman Engine")
            .and_then(|engine| engine["Details"]["MinAPIVersion"].as_str())
            .map(|s| s.to_string()).unwrap();
        it.api_version = format!("/v{}", version);
        Ok(it)
    }

    pub async fn request(&mut self, req: Request<Full<Bytes>>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut response = self.client.request(req).await?;

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

    pub async fn get(&mut self, path: &str, body: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let path_with_prefix = format!("{}{}", self.api_version, path);
        let url: Uri = Uri::new(self.path.as_str(), path_with_prefix.as_str()).into();

        let payload = Full::new(Bytes::from(body));

        let req = Request::builder()
            .method(Method::GET)
            .uri(url)
            .header(CONTENT_TYPE, "application/json")
            .body(payload)?;

        self.request(req).await
    }

    pub async fn post(&mut self, path: &str, body: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let path_with_prefix = format!("{}{}", self.api_version, path);
        let url: Uri = Uri::new(self.path.as_str(), path_with_prefix.as_str()).into();

        let payload = Full::new(Bytes::from(body));

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(CONTENT_TYPE, "application/json")
            .body(payload)?;

        self.request(req).await
    }

    pub async fn delete(&mut self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let path_with_prefix = format!("{}{}", self.api_version, path);
        let url: Uri = Uri::new(self.path.as_str(), path_with_prefix.as_str()).into();

        let payload = Full::new(Bytes::from(Vec::new()));

        let req = Request::builder()
            .method(Method::DELETE)
            .uri(url)
            .header(CONTENT_TYPE, "application/json")
            .body(payload)?;

        self.request(req).await
    }


    pub async fn create_container(&mut self, name: Option<&str>, image: &str) -> Result<String, Box<dyn std::error::Error>> {
        let spec = json!({
            "image": image,
            "name": name,
            "stdin": true,
            "tty": true,
        });

        let payload: Vec<u8> = spec.to_string().into();

        let resp = self.post("/libpod/containers/create", payload).await?;

        let result = String::from_utf8(resp)?;
        Ok(result)
    }

    pub async fn start_container(&mut self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let payload = Vec::<u8>::new();

        let path = format!("/libpod/containers/{}/start", name);
        let resp = self.post(path.as_str(), payload).await?;

        let result = String::from_utf8(resp)?;
        Ok(result)
    }

    pub async fn stop_container(&mut self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let payload = Vec::<u8>::new();

        let path = format!("/libpod/containers/{}/stop", name);
        let resp = self.post(path.as_str(), payload).await?;

        let result = String::from_utf8(resp)?;
        Ok(result)
    }

    pub async fn delete_container(&mut self, name: &str, force: bool) -> Result<String, Box<dyn std::error::Error>> {
        let path = format!("/libpod/containers/{}?force={}", name, force);
        let resp = self.delete(path.as_str()).await?;

        let result = String::from_utf8(resp)?;
        Ok(result)
    }

}

#[cfg(test)]
mod tests {
    use crate::podman::Podman;

    #[tokio::test]
    async fn test_podman_ping() {
        let mut pm = Podman::new(None).await.unwrap();

        let result = pm.get("/libpod/_ping", Vec::new()).await.unwrap();
        let actual = std::str::from_utf8(result.as_slice()).unwrap();
        assert_eq!("OK", actual);
    }

    #[tokio::test]
    async fn test_create_start_stop_delete_container() {
        let mut pm = Podman::new(None).await.unwrap();

        let container_name = "arkade_test_container";

        let actual = pm.create_container(Some(container_name), "netutils").await.unwrap_or_default();
        assert_ne!("", actual);

        let actual = pm.start_container(container_name).await.unwrap_or_default();
        assert_eq!("", actual);

        let actual = pm.stop_container(container_name).await.unwrap_or_default();
        assert_eq!("", actual);

        let actual = pm.delete_container(container_name, true).await.unwrap_or_default();
        assert_ne!("", actual);
    }

}
