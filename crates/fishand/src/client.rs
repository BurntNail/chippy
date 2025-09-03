use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;

enum ClientState {
    Connected,
    Introduced {
        name: String
    },
    Closed,
}

pub struct Client {
    state: ClientState,
    addr: SocketAddr,
    global_msgs_to_send: Vec<EventToClient>,
    local_msgs_to_send: Vec<EventToClient>,
}

impl Client {
    pub fn new (addr: SocketAddr) -> Self {
        Self {
            state: ClientState::Connected,
            addr,
            global_msgs_to_send: vec![],
            local_msgs_to_send: vec![],
        }
    }
}

impl Display for Client {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.state {
            ClientState::Connected => write!(f, "[{}]", self.addr.ip()),
            ClientState::Introduced { name } => write!(f, "{name:?}"),
            ClientState::Closed => write!(f, "[closed]"),
        }
    }
}

impl Client {
    pub fn close (&mut self, reason: Option<CloseFrame>) {
        println!("{self} closing for {reason:?}");
        if let ClientState::Introduced {name} = &self.state {
            self.global_msgs_to_send.push(EventToClient::TxtSent {
                name: "SERVER".to_string(),
                content: format!("{name} left the server")
            });
        }
        self.state = ClientState::Closed;
    }
    
    pub fn should_quit (&self) -> bool {
        matches!(self.state, ClientState::Closed)
    }
    pub fn can_interact (&self) -> bool {
        matches!(self.state, ClientState::Introduced {..})
    }

    pub fn local_msgs_to_send (&mut self) -> impl Iterator<Item = EventToClient> {
        self.local_msgs_to_send.drain(..)
    }
    pub fn global_msgs_to_send (&mut self) -> impl Iterator<Item = EventToClient> {
        self.global_msgs_to_send.drain(..)
    }
    
    pub fn process_event (&mut self, evt: EventToServer) {
        match evt {
            EventToServer::Introduction { name } => {
                if matches!(self.state, ClientState::Connected) {
                    self.local_msgs_to_send.push(EventToClient::Introduced);
                    self.global_msgs_to_send.push(EventToClient::TxtSent {
                        name: "SERVER".to_string(),
                        content: format!("{name} joined the server")
                    });
                    self.state = ClientState::Introduced {name};
                }
            }
            EventToServer::SendMessage { msg } => {
                if self.can_interact() {
                    self.global_msgs_to_send.push(EventToClient::TxtSent {
                        name: self.to_string(),
                        content: msg,
                    });
                }
            }
        }
    }
}