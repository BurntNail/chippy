#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

#[allow(clippy::missing_panics_doc)] //in the name :)
pub fn eof_or_panic<T>(res: tokio::io::Result<T>) -> Option<T> {
    match res {
        Ok(n) => Some(n),
        Err(e) => {
            if e.kind() == ErrorKind::UnexpectedEof {
                //we quit :)
                None
            } else {
                panic!("{e:?}"); //FIXME: unwrap
            }
        }
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("unable to install color eyre");

    let (send_event, _) = broadcast::channel(16);
    let n_clients = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    while let Ok((stream, addr)) = listener.accept().await {
        let name = addr.to_string();
        
        let send_event = send_event.clone();
        let mut recv_event = send_event.subscribe();
        
        let (mut read_half, mut write_half) = stream.into_split();
        let disconnect = Arc::new(AtomicBool::new(false));
        let set_disconnect = disconnect.clone();

        let n_clients = n_clients.clone();
        println!("⬆️ {}", n_clients.fetch_add(1, Ordering::SeqCst) + 1);

        tokio::spawn(async move {
            let mut message_decoder = EventToServer::deser();

            loop {
                match message_decoder.wants_read() {
                    DesiredInput::Byte(one_byte) => {
                        let Some(read) = eof_or_panic(read_half.read_u8().await) else {
                            break;
                        };
                        *one_byte = read;
                        message_decoder.finish_bytes_for_writing(1);
                    }
                    DesiredInput::Bytes(many_bytes) => {
                        let Some(n) = eof_or_panic(read_half.read(many_bytes).await) else {
                            break;
                        };
                        message_decoder.finish_bytes_for_writing(n);
                    }
                    //FIXME: unwrap
                    DesiredInput::ProcessMe => {
                        message_decoder = match message_decoder.process().unwrap() {
                            FsmResult::Continue(deser) => deser,
                            FsmResult::Done(event) => {
                                println!("{name} recv {event:?}");
                                match event {
                                    EventToServer::SendMessage(content) => {
                                        let _ = send_event.send(EventToClient::TxtSent {
                                            name: name.clone(),
                                            content,
                                        });
                                    }
                                    EventToServer::Quit => {
                                        let _ = send_event.send(EventToClient::TxtSent {
                                            name: name.clone(),
                                            content: "LEFT SERVER".to_string(),
                                        });
                                        set_disconnect.store(true, Ordering::SeqCst);
                                        println!("⬇️ {}", n_clients.fetch_sub(1, Ordering::SeqCst) - 1);
                                        break;
                                    }
                                }
                                EventToServer::deser()
                            }
                        };
                    }
                    DesiredInput::Start => {
                        unreachable!()
                    }
                }
            }

        });

        tokio::spawn(async move {
            let mut msg_buffer = Vec::new();
            loop {
                if disconnect.load(Ordering::SeqCst) {
                    break;
                }
                
                while let Ok(msg) = recv_event.try_recv() {
                    msg_buffer.clear();
                    msg.ser_into(&mut msg_buffer); //avoid re-allocating every time
                    //possible issue - large message balloons it and not freed??

                    write_half.write_all(&msg_buffer).await.unwrap(); //FIXME: unwrap
                }
            }
        });
    }

    Ok(())
}
