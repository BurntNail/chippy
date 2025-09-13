use crate::worker_thread::IOThread;
use eframe::{App, Frame};
use egui::{Context, TextBuffer};
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;
use fishandchippy::game_types::player::Player;
use fishandchippy::game_types::pot::Pot;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct ChippyApp {
    io: IOThread,
    state: ChippyAppState,
}

//TODO: move this to the event enum?
enum MessageSender {
    Player(Uuid),
    Admin,
}

enum ChippyAppState {
    WaitingMenu {
        write_name_buffer: String,
        server_buffer: String,
    },
    ErrorHappened {
        error: String,
    },
    LoadedIn {
        send_msg_buffer: String,
        msgs_so_far: Vec<(MessageSender, String)>,
        our_uuid: Uuid,
        players: HashMap<Uuid, Player>,
        pot: Pot,
    },
}

impl Default for ChippyAppState {
    fn default() -> Self {
        Self::WaitingMenu {
            write_name_buffer: String::from(""),
            server_buffer: String::from(""),
        }
    }
}

impl ChippyApp {
    pub fn new() -> color_eyre::Result<Self> {
        Ok(Self {
            io: IOThread::new(),
            state: ChippyAppState::default(),
        })
    }

    fn get_my_player_data(&self) -> Option<&Player> {
        if let ChippyAppState::LoadedIn {
            players, our_uuid, ..
        } = &self.state
        {
            players.get(our_uuid)
        } else {
            None
        }
    }

    fn game_update(&mut self) {
        let mut reqs_to_send = HashSet::new();
        let events = match self.io.poll_and_get_events() {
            Ok(events) => events,
            Err(e) => {
                self.state = ChippyAppState::ErrorHappened {
                    error: e.to_string(),
                };
                return;
            }
        };

        for result in events {
            match result {
                EventToClient::TxtSent(uuid, content) => {
                    if let ChippyAppState::LoadedIn {
                        msgs_so_far,
                        players,
                        ..
                    } = &mut self.state
                    {
                        msgs_so_far.push((MessageSender::Player(uuid), content));
                        if !players.contains_key(&uuid) {
                            reqs_to_send.insert(EventToServer::GetSpecificPlayer(uuid));
                        }
                    }
                }
                EventToClient::AdminMsg(content) => {
                    if let ChippyAppState::LoadedIn { msgs_so_far, .. } = &mut self.state {
                        msgs_so_far.push((MessageSender::Admin, content));
                    }
                }
                EventToClient::Introduced(uuid) => {
                    info!("User state now loaded");
                    self.state = ChippyAppState::LoadedIn {
                        send_msg_buffer: "".to_string(),
                        msgs_so_far: vec![],
                        our_uuid: uuid,
                        players: HashMap::new(),
                        pot: Pot::default(),
                    };
                    reqs_to_send.insert(EventToServer::GetStartInformation);
                }
                EventToClient::Pot(new_pot) => {
                    if let ChippyAppState::LoadedIn { pot, players, .. } = &mut self.state {
                        *pot = new_pot;

                        for uuid in pot.ready_to_put_in.keys() {
                            if !players.contains_key(uuid) {
                                reqs_to_send.insert(EventToServer::GetSpecificPlayer(*uuid));
                            }
                        }
                    }
                }
                EventToClient::AllPlayers(new_players) => {
                    if let ChippyAppState::LoadedIn { players, .. } = &mut self.state {
                        *players = new_players;
                    }
                }
                EventToClient::SpecificPlayer(uuid, player) => {
                    if let ChippyAppState::LoadedIn { players, .. } = &mut self.state {
                        players.insert(uuid, player);
                    }
                }
            }
        }
        self.io.send_reqs(&reqs_to_send);
    }
}

impl App for ChippyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        ctx.request_repaint();
        self.game_update();

        let mut needs_to_reset = false;
        let mut error = None;
        match &mut self.state {
            ChippyAppState::WaitingMenu {
                write_name_buffer,
                server_buffer,
            } => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if self.io.is_disconnected() {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Server: ");
                                ui.text_edit_singleline(server_buffer);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Name: ");
                                ui.text_edit_singleline(write_name_buffer);
                            });
                            if ui.button("Connect").clicked() {
                                if let Err(e) = self
                                    .io
                                    .connect(server_buffer.take(), write_name_buffer.take())
                                {
                                    error!("Error connecting to server: {e:?}");
                                    error = Some(e.to_string());
                                }
                            }
                        });
                    } else if self.io.is_waiting() {
                        ui.horizontal(|ui| {
                            ui.label("Connecting...");
                            ui.spinner();
                        });
                    }
                });
            }
            ChippyAppState::ErrorHappened { error } => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Error connecting to server: ");
                    ui.code(error);
                    if ui.button("Try again?").clicked() {
                        needs_to_reset = true;
                    }
                });
            }
            ChippyAppState::LoadedIn {
                send_msg_buffer,
                msgs_so_far,
                our_uuid,
                players,
                pot,
            } => {
                egui::TopBottomPanel::bottom("send msg").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(send_msg_buffer);
                        if ui.button("Send Msg").clicked() {
                            self.io.send_req(EventToServer::SendMessage {
                                content: std::mem::take(send_msg_buffer),
                            });
                        }
                        if ui.button("Quit").clicked() {
                            needs_to_reset = true;
                        }
                    });
                });
                egui::SidePanel::right("view msg").show(ctx, |ui| {
                    for (sender, content) in msgs_so_far {
                        match sender {
                            MessageSender::Player(uuid) => {
                                if let Some(player) = players.get(uuid) {
                                    if *our_uuid == *uuid {
                                        ui.label(format!("{player} (you): {content}"));
                                    } else {
                                        ui.label(format!("{player}: {content}"));
                                    }
                                }
                            } //should always be OK but whatever
                            MessageSender::Admin => {
                                ui.label(format!("SERVER: {content}"));
                            }
                        }
                    }
                });
                egui::SidePanel::left("game info").show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.label("Players: ");
                        for (uuid, player) in players.iter() {
                            if *our_uuid == *uuid {
                                ui.label(format!("\t{player} (you) - {}", player.balance));
                            } else {
                                ui.label(format!("\t{player} - {}", player.balance));
                            }
                        }
                        ui.label("Pot: ");
                        ui.label(format!("\tCurrent Value: {}", pot.current_value));
                        for (uuid, amt) in &pot.ready_to_put_in {
                            if let Some(player) = players.get(uuid) {
                                if *our_uuid == *uuid {
                                    ui.label(format!("\t{player} (you)- {}", amt));
                                } else {
                                    ui.label(format!("\t{player} - {}", amt));
                                }
                            } //should always be OK but whatever
                        }
                    });
                });
            }
        }

        if needs_to_reset {
            self.io.quit();
            self.state = ChippyAppState::default();
        } else if let Some(error) = error {
            self.io.quit();
            self.state = ChippyAppState::ErrorHappened { error };
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.io.quit();
    }
}
