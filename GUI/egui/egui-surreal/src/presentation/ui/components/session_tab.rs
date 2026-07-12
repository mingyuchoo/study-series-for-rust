use crate::presentation::controllers::AppController;
use egui::{Color32,
           RichText};

pub fn show_session_tab(controller: &mut AppController, ui: &mut egui::Ui) {
    ui.heading("Session Information");
    ui.separator();

    ui.vertical(|ui| {
        if ui
            .add_enabled(!controller.state.is_loading, egui::Button::new("Refresh Session Data"))
            .clicked()
        {
            controller.get_session();
        }

        ui.add_space(10.0);

        ui.label(RichText::new("Session Details").strong());
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            if controller.state.session_info.is_empty() {
                ui.label(RichText::new("No session data loaded. Click 'Refresh Session Data' to load.").color(Color32::GRAY));
            } else {
                ui.text_edit_multiline(&mut controller.state.session_info);
            }
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("About Sessions").strong());
            ui.label("Session data shows your current authentication state,");
            ui.label("including user information and permissions.");
            ui.label("This is useful for debugging authentication issues.");
        });
    });
}
