use crate::Table;
use crate::client::Client;
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::{EventToServer, ServerEventDeserer};
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use futures::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::{Bytes, Message};

pub async fn handle_connection(
    peer: SocketAddr,
    stream: TcpStream,
    global_send_event: Sender<EventToClient>,
    mut global_recv_event: Receiver<EventToClient>,
    table: Arc<RwLock<Table>>,
) -> color_eyre::Result<()> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");
    println!("New WebSocket connection: {peer}");

    let mut msgs_to_process: VecDeque<Message> = VecDeque::new();
    let mut client = Client::new(peer);

    loop {
        for msg in client.local_msgs_to_send() {
            ws_stream
                .send(Message::Binary(Bytes::from_owner(msg.ser().1)))
                .await?;
        }
        for msg in client.global_msgs_to_send() {
            if global_send_event.send(msg).is_err() {
                eprintln!("Error sending global message...");
            }
        }
        if client.should_quit() {
            break;
        }

        tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    None => {
                        client.close(None, &table).await;
                    }
                    Some(Err(e)) => {
                        eprintln!("Error receiving message from {client}: {e}");
                        return Err(e.into());
                    }
                    Some(Ok(msg)) => {
                        msgs_to_process.push_back(msg);
                    }
                }
            },
            evt = global_recv_event.recv() => {
                if let Ok(evt) = evt {
                    ws_stream.send(Message::Binary(Bytes::from_owner(evt.ser().1))).await?;
                }
            }
        }

        while let Some(to_be_processed) = msgs_to_process.pop_front() {
            match to_be_processed {
                Message::Binary(binary) => {
                    let mut binary = binary.into_iter().peekable();
                    let mut deserer = EventToServer::deser();
                    loop {
                        //TODO: no bytes left but not done?
                        if matches!(deserer, ServerEventDeserer::Start(_))
                            && binary.peek().is_none()
                        {
                            break;
                        }

                        match deserer.wants_read() {
                            DesiredInput::Byte(space) => {
                                if let Some(byte) = binary.next() {
                                    *space = byte;
                                    deserer.finish_bytes_for_writing(1);
                                }
                            }
                            DesiredInput::Bytes(space) => {
                                let mut n = 0;
                                for next_space in space {
                                    let Some(byte) = binary.next() else {
                                        break;
                                    };
                                    *next_space = byte;

                                    n += 1;
                                }
                                deserer.finish_bytes_for_writing(n);
                            }
                            DesiredInput::ProcessMe => {
                                deserer = match deserer.process()? {
                                    FsmResult::Continue(cont) => cont,
                                    FsmResult::Done(evt) => {
                                        println!("{client} sent {evt:?}");
                                        client.process_event(evt, &table).await;
                                        EventToServer::deser()
                                    }
                                }
                            }
                            DesiredInput::Extra => unreachable!(),
                        }
                    }
                }
                Message::Close(close) => {
                    client.close(close.as_ref(), &table).await;
                }
                unexpected => {
                    eprintln!("received unexpected msg from {client}: {unexpected:?}");
                }
            }
        }
    }

    Ok(())
}
