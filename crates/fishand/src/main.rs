#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

mod conn;

use tokio::net::TcpListener;
use tokio::sync::broadcast;
use crate::conn::handle_connection;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("unable to install color eyre");

    let (send_event, _) = broadcast::channel(16);
    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    while let Ok((stream, addr)) = listener.accept().await {
        let name = addr.to_string();
        let send_event = send_event.clone();
        let recv_event = send_event.subscribe();
        
        tokio::task::spawn(async move {
            if let Err(e) = handle_connection(name, addr, stream, send_event, recv_event).await {
                eprintln!("Error serving conn: {e}");
            }
        });
    }

    Ok(())
}
