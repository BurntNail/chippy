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
    pub fn new (server: String) -> color_eyre::Result<Self> {
        Ok(Self {
            send_msg_buffer: String::new(),
            msgs_so_far: Vec::new(),
            io: IOThread::new(server)?,
        })
    }
}

impl App for ChippyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        ctx.request_repaint();
        
        egui::TopBottomPanel::bottom("send msg").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.send_msg_buffer);
                if ui.button("Send Msg").clicked() {
                    self.io.send_msg(std::mem::take(&mut self.send_msg_buffer));
                }
                if ui.button("Quit").clicked() {
                    self.io.quit(true);
                    unimplemented!("quit logic");
                }
            });
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            for (sender, content) in &self.msgs_so_far {
                ui.label(format!("[{sender}]: {content}"));
            }
        });

        for result in self.io.get_events().unwrap() {
            match result {
                EventToClient::TxtSent { name, content } => self.msgs_so_far.push((name, content)),
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.io.quit(true);
    }
}