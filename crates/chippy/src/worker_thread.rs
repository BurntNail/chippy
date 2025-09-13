use ewebsock::{Options, WsEvent, WsMessage, WsReceiver, WsSender};
use fishandchippy::events::client::{ClientEventDeserer, EventToClient};
use fishandchippy::events::server::EventToServer;
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use uuid::Uuid;

pub struct IOThread {
    state: IOThreadState,
}

enum IOThreadState { 
    Disconnected, 
    TryingToConnect {
        tx: WsSender,
        rx: WsReceiver,
        name: String
    },
    WaitingOnAcknowledgement {
        tx: WsSender,
        rx: WsReceiver
    },
    Connected {
        tx: WsSender,
        rx: WsReceiver,
        uuid: Uuid,
    }
}

impl IOThread {
    fn get_tx_rx (&mut self) -> Option<(&mut WsSender, &mut WsReceiver)> {
        match &mut self.state {
            IOThreadState::Disconnected => None,
            IOThreadState::TryingToConnect {tx, rx, ..} | IOThreadState::WaitingOnAcknowledgement {tx, rx} | IOThreadState::Connected {tx, rx, ..} => Some((tx, rx))
        }
    }
    
    pub fn new () -> Self {
        Self {
            state: IOThreadState::Disconnected
        }
    }
    
    pub fn is_disconnected (&self) -> bool {
        matches!(self.state, IOThreadState::Disconnected)
    }
    pub fn is_waiting(&self) -> bool {
        matches!(self.state, IOThreadState::WaitingOnAcknowledgement {..} | IOThreadState::TryingToConnect {..})
    }
    pub fn is_connected (&self) -> Option<Uuid> {
        match self.state {
            IOThreadState::Connected {uuid, ..} => Some(uuid),
            _ => None
        }
    }
    
    pub fn connect (&mut self, server: String, name: String) -> color_eyre::Result<()> {
        let (tx, rx) = ewebsock::connect(server, Options::default()).map_err(string_to_eyre)?;
       
        self.state = IOThreadState::TryingToConnect {
            tx,
            rx,
            name
        };
        
        Ok(())
    }
    
    pub fn quit (&mut self) {
        let tx_and_rx = match std::mem::replace(&mut self.state, IOThreadState::Disconnected) {
            IOThreadState::Disconnected => None,
            IOThreadState::TryingToConnect {tx, rx, ..} => Some((tx, rx)),
            IOThreadState::WaitingOnAcknowledgement { tx, rx } => Some((tx, rx)),
            IOThreadState::Connected { tx, rx, .. } => Some((tx, rx)),
        };
        if let Some((mut tx, _rx)) = tx_and_rx {
            //_rx just in case i need to do anything with it in the future
            tx.close();
        }

        //TODO: more quit logic?
    }
    
    pub fn send_req (&mut self, req: EventToServer) {
        if self.is_connected().is_some() && let Some((tx, _rx)) = self.get_tx_rx() {
            tx.send(WsMessage::Binary(req.ser().1));
        }
    }
    pub fn send_reqs<'a> (&mut self, req: impl IntoIterator<Item = &'a EventToServer>) {
        if self.is_connected().is_some() && let Some((tx, _rx)) = self.get_tx_rx() {
            let mut output = vec![];
            req.into_iter().for_each(|e| e.ser_into(&mut output));
            if !output.is_empty() {
                tx.send(WsMessage::Binary(output));
            }
        }
    }
    
    pub fn poll_and_get_events(&mut self) -> color_eyre::Result<impl IntoIterator<Item = EventToClient>> {
        let Some((_tx, mut rx)) = self.get_tx_rx() else {
            return Ok(vec![]);
        };

        let mut evts = vec![];

        while let Some(to_be_processed) = rx.try_recv() {
            match to_be_processed {
                WsEvent::Opened => {
                    info!("Connection opened :)");
                    let IOThreadState::TryingToConnect {tx: mut new_tx, rx: new_rx, name} = std::mem::replace(&mut self.state, IOThreadState::Disconnected) else {
                        unreachable!();
                    };
                    
                    new_tx.send(WsMessage::Binary(EventToServer::Introduction {name}.ser().1));
                    self.state = IOThreadState::WaitingOnAcknowledgement {tx: new_tx, rx: new_rx};

                    let (_, new_new_rx) = self.get_tx_rx().unwrap();
                    rx = new_new_rx;
                }
                WsEvent::Error(uhoh) => {
                    return Err(string_to_eyre(uhoh));
                }
                WsEvent::Closed => {
                    self.quit();
                    return Ok(evts);
                }
                WsEvent::Message(msg) => {
                    info!("recevied {msg:?}");
                    match msg {
                        WsMessage::Binary(binary) => {
                            let mut binary = binary.into_iter().peekable();
                            let mut deserer = EventToClient::deser();

                            loop {
                                if binary.peek().is_none() && matches!(deserer, ClientEventDeserer::Start(_)) {
                                    info!("breaking @ {deserer:?}");
                                    break;
                                }

                                match deserer.wants_read() {
                                    DesiredInput::Byte(space) => {
                                        if let Some(byte) = binary.next() {
                                            *space = byte;
                                            info!("writing {byte:x}");
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

                                                if let EventToClient::Introduced(uuid) = evt {
                                                    info!("Acknowledgement received, joining server");
                                                    let IOThreadState::WaitingOnAcknowledgement {tx: new_tx, rx: new_rx} = std::mem::replace(&mut self.state, IOThreadState::Disconnected) else {
                                                        unreachable!()
                                                    };
                                                    
                                                    info!("IO now connected and introduced");
                                                    self.state = IOThreadState::Connected {tx: new_tx, rx: new_rx, uuid};

                                                    let (_, new_new_rx) = self.get_tx_rx().unwrap();
                                                    rx = new_new_rx;
                                                }
                                                
                                                info!("\tparsed as {evt:?}");
                                                evts.push(evt);
                                                EventToClient::deser()
                                            }
                                        }
                                    }
                                    DesiredInput::Extra => unreachable!()
                                }
                            }
                        }
                        unexpected => warn!("Received unexpected message: {unexpected:?}"),
                    }
                }
            }
        }
        
        Ok(evts)
    }
}

pub fn string_to_eyre (s: String) -> color_eyre::Report {
    color_eyre::Report::msg(s)
}