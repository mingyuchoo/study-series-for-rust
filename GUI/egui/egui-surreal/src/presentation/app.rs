use crate::{domain::MessageType,
            presentation::{controllers::AppController,
                           ui::{AppTab,
                                show_auth_tab,
                                show_people_tab,
                                show_query_tab,
                                show_session_tab}}};
use egui::{Color32,
           RichText};

pub struct SurrealDbApp {
    controller: AppController,
}

impl SurrealDbApp {
    pub fn new(controller: AppController) -> Self {
        Self {
            controller,
        }
    }

    fn show_messages(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label(RichText::new("Messages").heading().color(Color32::WHITE));

        for message in &self.controller.state.messages {
            let color = match message.msg_type {
                | MessageType::Success => Color32::from_rgb(0, 200, 0),
                | MessageType::Error => Color32::from_rgb(200, 0, 0),
            };

            let elapsed = message.timestamp.elapsed().as_secs();
            ui.horizontal(|ui| {
                ui.label(RichText::new(&message.content).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{}s ago", elapsed)).small().color(Color32::GRAY));
                });
            });
        }
    }
}

impl eframe::App for SurrealDbApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll DB responses without painting
        self.controller.handle_response();

        // Keep UI responsive while async work is in flight
        if self.controller.state.is_loading {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Top panel with tabs and status
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("SurrealDB Manager");

                ui.separator();

                // Tab buttons
                if ui.selectable_label(self.controller.state.current_tab == AppTab::Session, "Session").clicked() {
                    self.controller.state.current_tab = AppTab::Session;
                }
                if ui
                    .selectable_label(self.controller.state.current_tab == AppTab::Authentication, "Auth")
                    .clicked()
                {
                    self.controller.state.current_tab = AppTab::Authentication;
                }
                if ui.selectable_label(self.controller.state.current_tab == AppTab::People, "People").clicked() {
                    self.controller.state.current_tab = AppTab::People;
                }
                if ui.selectable_label(self.controller.state.current_tab == AppTab::Query, "Query").clicked() {
                    self.controller.state.current_tab = AppTab::Query;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Connection status
                    let status_color = if self.controller.state.connection_status.contains("Connected") {
                        Color32::from_rgb(0, 200, 0)
                    } else {
                        Color32::from_rgb(200, 0, 0)
                    };
                    ui.label(RichText::new(&self.controller.state.connection_status).color(status_color));

                    if self.controller.state.is_loading {
                        ui.spinner();
                    }
                });
            });
        });

        // Bottom panel for messages (before CentralPanel — panels must be added before
        // central)
        egui::Panel::bottom("bottom_panel").show(ui, |ui| {
            self.show_messages(ui);
        });

        // Main content area
        egui::CentralPanel::default().show(ui, |ui| match self.controller.state.current_tab {
            | AppTab::People => show_people_tab(&mut self.controller, ui),
            | AppTab::Authentication => show_auth_tab(&mut self.controller, ui),
            | AppTab::Query => show_query_tab(&mut self.controller, ui),
            | AppTab::Session => show_session_tab(&mut self.controller, ui),
        });
    }
}
