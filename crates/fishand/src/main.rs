#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

mod conn;
mod client;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use fishandchippy::game_types::player::Player;
use fishandchippy::game_types::pot::Pot;
use crate::conn::handle_connection;

#[derive(Debug, Default, Clone)]
pub struct Table {
    pub pot: Pot,
    pub players: HashMap<Uuid, Player>
}


#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("unable to install color eyre");

    let (send_event, _) = broadcast::channel(16);
    let table = Arc::new(RwLock::new(Table::default()));
    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    while let Ok((stream, addr)) = listener.accept().await {
        let send_event = send_event.clone();
        let recv_event = send_event.subscribe();
        let table = table.clone();
        
        tokio::task::spawn(async move {
            if let Err(e) = handle_connection(addr, stream, send_event, recv_event, table).await {
                eprintln!("Error serving conn: {e}");
            }
        });
    }

    Ok(())
}
