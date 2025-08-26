use std::net::ToSocketAddrs;
use eframe::{App, Frame};
use egui::Context;
use fishandchippy::events::client::EventToClient;
use crate::worker_thread::IOThread;

pub struct ChippyApp {
    send_msg_buffer: String,
    msgs_so_far: Vec<(String, String)>,
    io: IOThread
}

impl ChippyApp {
    pub fn new (server: impl ToSocketAddrs) -> Self {
        Self {
            send_msg_buffer: String::new(),
            msgs_so_far: Vec::new(),
            io: IOThread::new(server),
        }
    }
}

impl App for ChippyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::TopBottomPanel::bottom("send msg").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.send_msg_buffer);
                if ui.button("Send Msg").clicked() {
                    self.io.send_msg(std::mem::take(&mut self.send_msg_buffer));
                }
            });
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            for (sender, content) in &self.msgs_so_far {
                ui.label(format!("[{sender}]: {content}"));
            }
        });
        
        for result in self.io.try_iter() {
            match result {
                EventToClient::TxtSent { name, content } => self.msgs_so_far.push((name, content)),
                EventToClient::ServerEnd => todo!()
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.io.quit();
    }
}