use eframe::{App, Frame};
use egui::{Context, TextBuffer};
use fishandchippy::events::client::EventToClient;
use crate::worker_thread::{IOThread, IntroductionState};

pub struct ChippyApp {
    io: IOThread,
    state: ChippyAppState,
}

enum ChippyAppState {
    WaitingMenu {
        write_name_buffer: String,
    },
    LoadedIn {
        send_msg_buffer: String,
        msgs_so_far: Vec<(String, String)>,
    }
}

impl ChippyApp {
    pub fn new (server: String) -> color_eyre::Result<Self> {
        Ok(Self {
            io: IOThread::new(server)?,
            state: ChippyAppState::WaitingMenu {
                write_name_buffer: String::new(),
            },
        })
    }
}

impl App for ChippyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        ctx.request_repaint();
        
        match &mut self.state {
            ChippyAppState::WaitingMenu {write_name_buffer} => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    match self.io.intro_state() {
                        IntroductionState::Needed => {
                            ui.vertical_centered(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Enter Name: ");
                                    ui.text_edit_singleline(write_name_buffer);
                                });
                                if ui.button("Connect").clicked() {
                                    self.io.send_introduction(write_name_buffer.take());
                                }
                            });
                        }
                        IntroductionState::Sent => {
                            ui.horizontal_centered(|ui| {
                                ui.label("Connecting...");
                                ui.spinner();
                            });
                        }
                        IntroductionState::Confirmed => unreachable!(),
                    }
                });
            }
            ChippyAppState::LoadedIn { send_msg_buffer, msgs_so_far } => {
                egui::TopBottomPanel::bottom("send msg").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(send_msg_buffer);
                        if ui.button("Send Msg").clicked() {
                            self.io.send_msg(std::mem::take(send_msg_buffer));
                        }
                        if ui.button("Quit").clicked() {
                            self.io.quit(true);
                            unimplemented!("quit logic");
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
        

        for result in self.io.get_events().unwrap() {
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