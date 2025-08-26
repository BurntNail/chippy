use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use http::{Request, Response, StatusCode};
use hyper::body::{Bytes, Incoming};
use hyper::service::Service;
use hyper_util::rt::TokioIo;
use soketto::handshake::http::is_upgrade_request;
use soketto::handshake::http::Server;
use futures::io::{BufReader, BufWriter};
use http_body_util::Full;
use hyper::upgrade::Upgraded;
use tokio::sync::broadcast::{Sender, Receiver};
use tokio::task::JoinHandle;
use fishandchippy::events::client::EventToClient;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use fishandchippy::events::server::EventToServer;
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

fn empty_with_code(code: StatusCode) -> Result<Response<Full<Bytes>>, http::Error> {
    Response::builder().status(code).body(Full::default())
}
type WSSender = soketto::Sender<BufReader<BufWriter<Compat<TokioIo<Upgraded>>>>>;
type WSReceiver = soketto::Receiver<BufReader<BufWriter<Compat<TokioIo<Upgraded>>>>>;

pub struct ServerService {
    send_event: Sender<EventToClient>,
    name: String,
}

impl ServerService {
    pub fn new (send_event: Sender<EventToClient>, name: String) -> Self {
        Self {
            send_event, name
        }
    }
}

impl Service<Request<Incoming>> for ServerService {
    type Response = Response<Full<Bytes>>;
    type Error = http::Error;
    type Future = std::future::Ready<Result<Response<Full<Bytes>>, http::Error>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        if is_upgrade_request(&req) {
            let mut handshake_server = Server::new();
            match handshake_server.receive_request(&req) {
                Ok(_rsp) => {
                    let send_event = self.send_event.clone();
                    let recv_event = send_event.subscribe();
                    let name = self.name.clone();

                    let _handle: JoinHandle<color_eyre::Result<()>> = tokio::task::spawn(async move {
                        let stream = hyper::upgrade::on(req).await?;
                        let io = TokioIo::new(stream);
                        let stream = BufReader::new(BufWriter::new(io.compat()));
                        let (ws_sender, ws_receiver) = handshake_server.into_builder(stream).finish();
                        
                        let stop = Arc::new(AtomicBool::new(false));
                        
                        let read = tokio::task::spawn(handle_reads(ws_receiver, send_event, stop.clone(), name));
                        let write = tokio::task::spawn(handle_writes(ws_sender, recv_event, stop));
                        
                        read.await??;
                        write.await??;

                        Ok(())
                    });
                    
                    std::future::ready(empty_with_code(StatusCode::OK))
                }
                Err(err) => {
                    eprintln!("couldn't upgrade connection: {err}");
                    std::future::ready(empty_with_code(StatusCode::INTERNAL_SERVER_ERROR))
                }
            }
        } else {
            //TODO: return default page?
            std::future::ready(empty_with_code(StatusCode::OK))
        }
    }
}

async fn handle_reads (mut ws_receiver: WSReceiver, send_event: Sender<EventToClient>, stop: Arc<AtomicBool>, name: String) -> color_eyre::Result<()> {
    let mut message_decoder = EventToServer::deser();
    let mut to_be_processed = vec![];
    let mut needs_to_read = false;
    
    loop {
        if needs_to_read {
            ws_receiver.receive_data(&mut to_be_processed).await?;
            needs_to_read = false;
        }
        
        match message_decoder.wants_read() {
            DesiredInput::Byte(one_byte) => {
                if to_be_processed.is_empty() {
                    needs_to_read = true;
                } else {
                    *one_byte = to_be_processed.remove(0);
                    message_decoder.finish_bytes_for_writing(1);
                }
            }
            DesiredInput::Bytes(many_bytes) => {
                if to_be_processed.is_empty() {
                    needs_to_read = true;
                } else {
                    let mut n = 0;
                    for (i, b) in to_be_processed.drain(..many_bytes.len()).enumerate() {
                        many_bytes[i] = b;
                        n += 1;
                    }
                    message_decoder.finish_bytes_for_writing(n);
                }
            }
            DesiredInput::ProcessMe => {
                message_decoder = match message_decoder.process()? {
                    FsmResult::Continue(deser) => deser,
                    FsmResult::Done(event) => {
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
                                stop.store(true, Ordering::SeqCst);
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
        
        if stop.load(Ordering::SeqCst) {
            break;
        }
    }
    
    Ok(())
}

async fn handle_writes (mut sender: WSSender, mut recv_event: Receiver<EventToClient>, stop: Arc<AtomicBool>) -> color_eyre::Result<()> {
    let mut msg_buffer = Vec::new();
    
    loop {
        while let Ok(msg) = recv_event.try_recv() {
            msg_buffer.clear();
            msg.ser_into(&mut msg_buffer); //avoid re-allocating every time
            //possible issue - large message balloons it and not freed??
            
            sender.send_binary(&msg_buffer).await?;
        }
        
        if stop.load(Ordering::SeqCst) {
            break;
        }
    }
    
    Ok(())
}