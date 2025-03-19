mod podman;

use http_body_util::{BodyExt};
use hyperlocal::{UnixClientExt};
use tokio::io::{AsyncWriteExt as _};
use crate::podman::Podman;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    Ok(())
}
