mod podman;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use tokio::io::{self, AsyncWriteExt as _};
use crate::podman::Podman;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    Ok(())
}

#[tokio::test]
async fn test_podman_ping() -> Result<(), Box<dyn std::error::Error>> {
    let mut pm = Podman::new(None);

    let result = pm.get("/v4.0.0/libpod/_ping", None).await?;

    let view = std::str::from_utf8(result.as_slice()).unwrap();

    println!("{}", view);

    Ok(())
}
