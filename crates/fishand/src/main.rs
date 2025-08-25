use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

struct DecOnDrop(Arc<AtomicUsize>);
impl Drop for DecOnDrop {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn eof_or_panic<T> (res: tokio::io::Result<T>) -> Option<T> {
    match res {
        Ok(n) => Some(n),
        Err(e) => if e.kind() == ErrorKind::UnexpectedEof {
            //we quit :)
            None
        } else {
            panic!("{e:?}"); //FIXME: unwrap
        }
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("unable to install color eyre");

    let (send_event, _) = broadcast::channel(16);
    let n_clients = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    while let Ok((mut stream, addr)) = listener.accept().await {
        let n_clients = n_clients.clone();
        let name = addr.to_string();
        let send_event = send_event.clone();
        let mut recv_event = send_event.subscribe();

        let new_n_clients = n_clients.fetch_add(1, Ordering::SeqCst) + 1;
        println!("[{name}] connected, now {new_n_clients}");

        tokio::spawn(async move {
            let _ = DecOnDrop(n_clients);
            let mut message_decoder = EventToServer::deser();
            let mut msg_buffer = Vec::new();
            loop {
                match message_decoder.wants_read() {
                    DesiredInput::Byte(one_byte) => {
                        let Some(read) = eof_or_panic(stream.read_u8().await) else {
                            break;
                        };
                        *one_byte = read;
                        message_decoder.finish_bytes_for_writing(1);
                    }
                    DesiredInput::Bytes(many_bytes) => {
                        let Some(n) = eof_or_panic(stream.read(many_bytes).await) else {
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
                                            content
                                        });
                                    },
                                    EventToServer::Quit => {
                                        println!("{name} disconnected");
                                        let _ = send_event.send(EventToClient::TxtSent {
                                            name: name.clone(),
                                            content: "LEFT SERVER".to_string(),
                                        });
                                        break;
                                    },
                                }
                                EventToServer::deser()
                            }
                        };
                    }
                    DesiredInput::Start => {
                        unreachable!()
                    }
                }

                while let Ok(msg) = recv_event.try_recv() {
                    msg_buffer.clear();
                    let _ = msg.ser_into(&mut msg_buffer); //avoid re-allocating every time
                    //possible issue - large message balloons it and not freed??

                    stream.write_all(&msg_buffer).await.unwrap(); //FIXME: unwrap
                }
            }
        });
    }



    Ok(())
}
