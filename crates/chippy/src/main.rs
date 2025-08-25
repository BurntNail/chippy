#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use std::io::ErrorKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc::channel;

#[allow(clippy::missing_panics_doc)] //in the name :)
pub async fn eof_or_panic<F, T>(f: F) -> Option<T>
where
    F: Future<Output = tokio::io::Result<T>>,
{
    match f.await {
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
#[allow(clippy::too_many_lines)]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("failed to install colour eyre");

    let url = "localhost";
    let port = 8080;
    println!("[client]: waiting to connect");
    let stream = TcpStream::connect(format!("{url}:{port}")).await?;
    println!("[client]: connected");

    let (send_event, mut recv_event) = channel(1);
    let (send_stop, _) = broadcast::channel(1);

    tokio::task::spawn({
        async move {
            let mut input = [0; 1024]; //TODO: more flexible?
            loop {
                match tokio::io::stdin().read(&mut input).await {
                    Ok(n) => match String::from_utf8(input[0..n].to_vec()) {
                        Ok(message) => {
                            let message = message.trim().to_string();
                            if message == ":q" {
                                let _ = send_event.send(EventToServer::Quit).await;
                                break;
                            }
                            let _ = send_event.send(EventToServer::SendMessage(message)).await;
                        }
                        Err(e) => {
                            eprintln!("Error reading from stdin: {e}");
                            let _ = send_event.send(EventToServer::Quit).await;
                            break;
                        }
                    },
                    Err(e) => {
                        eprintln!("Error reading from stdin: {e}");
                        let _ = send_event.send(EventToServer::Quit).await;
                        break;
                    }
                }
            }
        }
    });

    let (mut reader, mut writer) = stream.into_split();

    let read_half = tokio::spawn({
        let send_stop = send_stop.clone();
        let mut recv_stop = send_stop.subscribe();
        let mut message_decoder = EventToClient::deser();

        async move {
            loop {
                match message_decoder.wants_read() {
                    DesiredInput::Byte(one_byte) => {
                        let Some(read) = eof_or_panic(reader.read_u8()).await else {
                            break;
                        };
                        *one_byte = read;
                        message_decoder.finish_bytes_for_writing(1);
                    }
                    DesiredInput::Bytes(many_bytes) => {
                        let Some(n) = eof_or_panic(reader.read(many_bytes)).await else {
                            break;
                        };
                        message_decoder.finish_bytes_for_writing(n);
                    }
                    DesiredInput::ProcessMe => {
                        message_decoder = match message_decoder.process().unwrap() {
                            //FIXME: unwrap
                            FsmResult::Continue(decoder) => decoder,
                            FsmResult::Done(event) => {
                                match event {
                                    EventToClient::TxtSent { name, content } => {
                                        println!("[{name}]: {content}");
                                    }
                                    EventToClient::ServerEnd => {
                                        println!("[server]: END");
                                        let _ = send_stop.send(());
                                        break;
                                    }
                                }

                                EventToClient::deser()
                            }
                        }
                    }
                    DesiredInput::Start => {
                        unreachable!()
                    }
                }

                if recv_stop.try_recv() == Ok(()) {
                    break;
                }
            }
        }
    });

    let write_half = tokio::spawn({
        let mut msg_buffer = vec![];

        async move {
            loop {
                if let Ok(msg) = recv_event.try_recv() {
                    msg_buffer.clear();
                    msg.ser_into(&mut msg_buffer); //avoid re-allocating every time
                    //possible issue - large message balloons it and not freed??

                    writer.write_all(&msg_buffer).await.unwrap(); //FIXME: unwrap

                    if matches!(msg, EventToServer::Quit) {
                        let _ = send_stop.send(()).unwrap();
                        break;
                    }
                }
            }
        }
    });

    read_half.await?;
    write_half.await?;

    Ok(())
}
