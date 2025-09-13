use crate::Table;
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;
use fishandchippy::game_types::player::Player;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use uuid::Uuid;

enum ClientState {
    Connected,
    Introduced { uuid: Uuid },
    Closed,
}

pub struct Client {
    state: ClientState,
    addr: SocketAddr,
    global_msgs_to_send: Vec<EventToClient>,
    local_msgs_to_send: Vec<EventToClient>,
}

impl Client {
    pub const fn new(addr: SocketAddr) -> Self {
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
            ClientState::Introduced { uuid } => write!(f, "[{uuid}]"),
            ClientState::Closed => write!(f, "[closed]"),
        }
    }
}

impl Client {
    #[allow(clippy::significant_drop_tightening)]
    pub async fn close(&mut self, reason: Option<&CloseFrame>, table: &RwLock<Table>) {
        println!("{self} closing for {reason:?}");

        if let ClientState::Introduced { uuid, .. } = &self.state {
            let table = table.write().await;

            let quit_msg = table.players.get(uuid).map_or_else(
                || format!("{uuid:?} left the server"),
                |player| format!("{:?} left the server", player.name),
            );
            self.global_msgs_to_send
                .push(EventToClient::AdminMsg(quit_msg));

            table.pot.ready_to_put_in.remove(uuid);
            table.players.remove(uuid);
        }

        self.state = ClientState::Closed;
    }

    pub const fn should_quit(&self) -> bool {
        matches!(self.state, ClientState::Closed)
    }
    pub const fn can_interact(&self) -> Option<Uuid> {
        match self.state {
            ClientState::Introduced { uuid } => Some(uuid),
            _ => None,
        }
    }

    pub fn local_msgs_to_send(&mut self) -> impl Iterator<Item = EventToClient> {
        self.local_msgs_to_send.drain(..)
    }
    pub fn global_msgs_to_send(&mut self) -> impl Iterator<Item = EventToClient> {
        self.global_msgs_to_send.drain(..)
    }

    pub async fn process_event(&mut self, evt: EventToServer, table: &RwLock<Table>) {
        match evt {
            EventToServer::Introduction { name } => {
                if matches!(self.state, ClientState::Connected) {
                    let uuid = Uuid::new_v4();

                    self.local_msgs_to_send
                        .push(EventToClient::Introduced(uuid));
                    self.global_msgs_to_send
                        .push(EventToClient::AdminMsg(format!(
                            "\"{name}\" joined the server"
                        )));

                    table.write().await.players.insert(
                        uuid,
                        Player {
                            name: name.clone(),
                            balance: 0,
                        },
                    );

                    self.state = ClientState::Introduced { uuid };
                }
            }
            EventToServer::SendMessage { content } => {
                if let Some(uuid) = self.can_interact() {
                    self.global_msgs_to_send
                        .push(EventToClient::TxtSent(uuid, content));
                }
            }
            EventToServer::GetStartInformation => {
                let table = table.read().await;
                self.local_msgs_to_send
                    .push(EventToClient::AllPlayers(table.players.clone()));
                self.local_msgs_to_send
                    .push(EventToClient::Pot(table.pot.clone()));
            }
            EventToServer::GetSpecificPlayer(their_uuid) => {
                let table = table.read().await;
                if let Some(player) = table.players.get(&their_uuid).cloned() {
                    self.local_msgs_to_send
                        .push(EventToClient::SpecificPlayer(their_uuid, player));
                }
            }
            EventToServer::AddToPot(value) => {
                if let Some(uuid) = self.can_interact() {
                    let mut table = table.write().await;
                    if let Some(player) = table.players.get_mut(&uuid)
                        && let Some(new_balance) = player.balance.checked_sub(value)
                    {
                        player.balance = new_balance;
                        self.local_msgs_to_send
                            .push(EventToClient::SpecificPlayer(uuid, player.clone()));

                        *table.pot.ready_to_put_in.entry(uuid).or_default() += value;
                        self.global_msgs_to_send
                            .push(EventToClient::Pot(table.pot.clone()));
                    }
                }
            }
        }
    }
}
