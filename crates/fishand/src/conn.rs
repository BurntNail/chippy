use std::collections::VecDeque;
use std::net::SocketAddr;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast::{Sender, Receiver};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::{Message, Bytes};
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::{EventToServer, ServerEventDeserer};
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

pub async fn handle_connection(name: String, peer: SocketAddr, stream: TcpStream, send_event: Sender<EventToClient>, mut recv_event: Receiver<EventToClient>) -> color_eyre::Result<()> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");
    println!("New WebSocket connection: {peer}");
    
    let mut msgs_to_process: VecDeque<Message> = VecDeque::new();
    
    loop {
        tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    None => {
                        println!("No more messages from {name}");
                        break;
                    }
                    Some(Err(e)) => {
                        eprintln!("Error receiving message: {e}");
                        return Err(e.into());
                    }
                    Some(Ok(msg)) => {
                        msgs_to_process.push_back(msg);
                    }
                }
            },
            evt = recv_event.recv() => {
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
                        if matches!(deserer, ServerEventDeserer::Start(_)) && binary.peek().is_none() {
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
                                for i in 0..space.len() {
                                    let Some(byte) = binary.next() else {
                                        break;
                                    };
                                    space[i] = byte;

                                    n += 1;
                                }
                                deserer.finish_bytes_for_writing(n);
                            }
                            DesiredInput::ProcessMe => {
                                deserer = match deserer.process()? {
                                    FsmResult::Continue(cont) => cont,
                                    FsmResult::Done(evt) => {
                                        println!("{name} sent {evt:?}");
                                        match evt {
                                            EventToServer::SendMessage(content) => {
                                                send_event.send(EventToClient::TxtSent {
                                                    name: name.clone(),
                                                    content
                                                })?;
                                            }
                                        }

                                        EventToServer::deser()
                                    }
                                }
                            }
                            DesiredInput::Start => unreachable!()
                        }
                    }

                }
                Message::Close(close) => {
                    println!("{name} is closing: {close:?}");
                }
                unexpected => {
                    eprintln!("received unexpected msg from {name}: {unexpected:?}");
                }
            }
        }
    }
    
    Ok(())
}