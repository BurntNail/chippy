use eframe::{App, Frame};
use egui::{Context, TextBuffer};
use fishandchippy::events::client::EventToClient;
use fishandchippy::events::server::EventToServer;
use crate::worker_thread::{IOThread};

pub struct ChippyApp {
    io: IOThread,
    state: ChippyAppState,
}

enum ChippyAppState {
    WaitingMenu {
        write_name_buffer: String,
        server_buffer: String,
    },
    LoadedIn {
        send_msg_buffer: String,
        msgs_so_far: Vec<(String, String)>,
    }
}

impl ChippyApp {
    pub fn new () -> color_eyre::Result<Self> {
        Ok(Self {
            io: IOThread::new(),
            state: ChippyAppState::WaitingMenu {
                write_name_buffer: String::new(),
                server_buffer: String::new(),
            },
        })
    }
}

impl App for ChippyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        ctx.request_repaint();
        
        let mut needs_to_reset = false;
        match &mut self.state {
            ChippyAppState::WaitingMenu {write_name_buffer, server_buffer} => {
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
                                if let Err(e) = self.io.connect(server_buffer.take(), write_name_buffer.take()) {
                                    error!("Error connecting to server: {e:?}");
                                    //TODO: error handling :)
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
            ChippyAppState::LoadedIn { send_msg_buffer, msgs_so_far } => {                
                egui::TopBottomPanel::bottom("send msg").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(send_msg_buffer);
                        if ui.button("Send Msg").clicked() {
                            self.io.send_req(EventToServer::SendMessage {msg: std::mem::take(send_msg_buffer)});
                        }
                        if ui.button("Quit").clicked() {
                            needs_to_reset = true;
                        }
                    });
                });

                egui::CentralPanel::default().show(ctx, |ui| {
                    for (sender, content) in msgs_so_far {
                        ui.label(format!("{sender}: {content}"));
                    }
                });
            }
        }
        
        if needs_to_reset {
            self.io.quit(true);
            self.state = ChippyAppState::WaitingMenu {
                write_name_buffer: String::new(),
                server_buffer: String::new(),
            };
        }
        

        for result in self.io.poll_and_get_events().unwrap() {
            match result {
                EventToClient::TxtSent { name, content } => {
                    if let ChippyAppState::LoadedIn {msgs_so_far, ..} = &mut self.state {
                        msgs_so_far.push((name, content));
                    }
                },
                EventToClient::Introduced => {
                    self.state = ChippyAppState::LoadedIn {
                        send_msg_buffer: String::new(),
                        msgs_so_far: Vec::new(),
                    };
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.io.quit(true);
    }
}