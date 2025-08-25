use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryIter};
use std::thread::JoinHandle;
use egui::Atom;
use egui::debug_text::print;
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

pub enum IORequest {
    SendMsg(String),
    Quit
}

pub struct IOThread {
    request_sender: Sender<IORequest>,
    result_receiver: Receiver<EventToClient>,
    read_handle: JoinHandle<()>,
    write_handle: JoinHandle<()>,
}

impl IOThread {
    pub fn new (server: impl ToSocketAddrs) -> Self {
        let (request_tx, request_rx) = channel();
        let (result_tx, result_rx) = channel();
        
        let stop = Arc::new(AtomicBool::new(false));
        let set_stop = stop.clone();
        
        let mut read_stream = TcpStream::connect(server).unwrap();
        let mut write_stream = read_stream.try_clone().unwrap();
        
        let read_handle = std::thread::spawn(move || {
            read_stream.set_nonblocking(true).unwrap(); //FIXME: unwrap
            let mut parser = EventToClient::deser();
            let nonblocking_or_panic = |stream: &mut TcpStream, slice: &mut [u8]| {
                match stream.read(slice) {
                    Ok(n) => Some(n),
                    Err(e) => if e.kind() == ErrorKind::WouldBlock {
                        None
                    } else {
                        panic!("{e:?}"); //FIXME: panic
                    }
                }
            };

            loop {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                
                match parser.wants_read() {
                    DesiredInput::Byte(space) => {
                        let mut arr = [0; 1];
                        if let Some(n) = nonblocking_or_panic(&mut read_stream, &mut arr) {
                            if n == 1 {
                                *space = arr[0];
                            }

                            parser.finish_bytes_for_writing(n);
                        }
                    }
                    DesiredInput::Bytes(space) => {
                        if let Some(n) = nonblocking_or_panic(&mut read_stream, space) {
                            parser.finish_bytes_for_writing(n);
                        }
                    }
                    DesiredInput::ProcessMe => {
                        //FIXME: unwrap
                        parser = match parser.process().unwrap() {
                            FsmResult::Continue(parser) => parser,
                            FsmResult::Done(result) => {
                                println!("i did a thing :)");
                                let _ = result_tx.send(result).unwrap();
                                EventToClient::deser()
                            }
                        }
                    }
                    DesiredInput::Start => {
                        unreachable!()
                    }
                }
            }
        });
        
        let write_handle = std::thread::spawn(move || {
            let mut msg_buf = vec![];

            'outer: loop {
                for request in request_rx.try_iter() {
                    match request {
                        IORequest::SendMsg(msg) => {
                            msg_buf.clear();
                            EventToServer::SendMessage(msg).ser_into(&mut msg_buf);
                            write_stream.write_all(&msg_buf).unwrap();
                        }
                        IORequest::Quit => {
                            msg_buf.clear();
                            EventToServer::Quit.ser_into(&mut msg_buf);
                            write_stream.write_all(&msg_buf).unwrap();

                            set_stop.store(true, Ordering::SeqCst);
                            break 'outer;
                        }
                    }
                }
            }
        });
        
        
        Self {
            request_sender: request_tx,
            result_receiver: result_rx,
            read_handle,
            write_handle,
        }
    }
    
    pub fn quit (&self) {
        self.request_sender.send(IORequest::Quit).unwrap();
    }
    
    pub fn send_msg (&self, msg: String) {
        self.request_sender.send(IORequest::SendMsg(msg)).unwrap();
    } 
    
    pub fn try_iter (&self) -> TryIter<'_, EventToClient> {
        self.result_receiver.try_iter()
    }
}