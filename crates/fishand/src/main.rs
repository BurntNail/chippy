#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

mod svc;

use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use crate::svc::ServerService;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("unable to install color eyre");

    let (send_event, _) = broadcast::channel(16);
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    let http = http1::Builder::new();

    while let Ok((stream, addr)) = listener.accept().await {
        let name = addr.to_string();
        let io = TokioIo::new(stream);
        let svc = ServerService::new(send_event.clone(), name);
        let conn = http.serve_connection(io, svc).with_upgrades();
        
        tokio::task::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("Error serving conn: {e}")
            }
        });
    }

    Ok(())
}
