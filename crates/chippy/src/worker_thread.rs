use ewebsock::{Options, WsEvent, WsMessage, WsReceiver, WsSender};
use fishandchippy::events::client::{ClientEventDeserer, EventToClient};
use fishandchippy::events::server::EventToServer;
use fishandchippy::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

pub struct IOThread {
    tx: WsSender,
    rx: WsReceiver,
    waiting_for_conn: Option<Vec<EventToServer>>,
    intro_state: IntroductionState,
}

#[derive(Copy, Clone, Debug)]
pub enum IntroductionState {
    Needed,
    Sent,
    Confirmed,
}

impl IOThread {
    pub fn new (server: impl Into<String>) -> color_eyre::Result<Self> {
        let (tx, rx) = ewebsock::connect(server, Options::default()).map_err(string_to_eyre)?;

        Ok(Self {
            tx, rx,
            waiting_for_conn: Some(vec![]),
            intro_state: IntroductionState::Needed,
        })
    }
    
    pub fn quit (&mut self, needs_to_close_socket: bool) {
        if needs_to_close_socket {
            self.tx.close();
        }
        self.waiting_for_conn = Some(vec![]);
        //TODO: quit logic
    }
    
    pub fn intro_state (&self) -> IntroductionState {
        self.intro_state
    }
    pub fn send_introduction (&mut self, name: String) {
        if matches!(self.intro_state, IntroductionState::Needed) {
            self.intro_state = IntroductionState::Sent;
            self.send_req(EventToServer::Introduction {name});
        }
    }
    
    pub fn send_msg (&mut self, msg: String) {
        self.send_req(EventToServer::SendMessage {msg});
    }
    fn send_req (&mut self, req: EventToServer) {
        if let Some(list) = self.waiting_for_conn.as_mut() {
            list.push(req);
            return;
        }

        self.tx.send(WsMessage::Binary(req.ser().1));
    }
    
    pub fn get_events(&mut self) -> color_eyre::Result<impl IntoIterator<Item = EventToClient>> {
        let mut evts = vec![];

        while let Some(to_be_processed) = self.rx.try_recv() {
            match to_be_processed {
                WsEvent::Opened => {
                    info!("Connection opened :)");
                    for msg in self.waiting_for_conn.take().unwrap_or_default() {
                        self.tx.send(WsMessage::Binary(msg.ser().1));
                    }
                }
                WsEvent::Error(uhoh) => {
                    return Err(string_to_eyre(uhoh));
                }
                WsEvent::Closed => {
                    self.quit(false);
                }
                WsEvent::Message(msg) => {
                    match msg {
                        WsMessage::Binary(binary) => {
                            let mut binary = binary.into_iter().peekable();
                            let mut deserer = EventToClient::deser();

                            loop {
                                if matches!(deserer, ClientEventDeserer::Start(_)) && binary.peek().is_none() {
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
                                                if matches!(evt, EventToClient::Introduced) {
                                                    self.intro_state = IntroductionState::Confirmed;
                                                }
                                                
                                                evts.push(evt);
                                                EventToClient::deser()
                                            }
                                        }
                                    }
                                    DesiredInput::Start => unreachable!()
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