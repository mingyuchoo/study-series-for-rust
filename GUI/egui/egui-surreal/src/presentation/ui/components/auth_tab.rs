use crate::presentation::controllers::AppController;
use egui::{Color32,
           RichText};

pub fn show_auth_tab(controller: &mut AppController, ui: &mut egui::Ui) {
    ui.heading("Authentication");
    ui.separator();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.label(RichText::new("Create New User").strong());
                if ui
                    .add_enabled(!controller.state.is_loading, egui::Button::new("Generate Random User"))
                    .clicked()
                {
                    controller.sign_up();
                }
                ui.label(RichText::new("Tip: Creates a user with random credentials").small().color(Color32::GRAY));
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(RichText::new("Sign In as User").strong());
                ui.horizontal(|ui| {
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut controller.state.auth_username);
                });
                ui.horizontal(|ui| {
                    ui.label("Password:");
                    ui.text_edit_singleline(&mut controller.state.auth_password);
                });

                let signin_enabled =
                    !controller.state.auth_username.trim().is_empty() && !controller.state.auth_password.trim().is_empty() && !controller.state.is_loading;

                if ui.add_enabled(signin_enabled, egui::Button::new("Sign In")).clicked() {
                    let username = controller.state.auth_username.trim().to_string();
                    let password = controller.state.auth_password.trim().to_string();
                    controller.sign_in(username, password);
                }
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(RichText::new("Admin Access").strong());
                if ui.add_enabled(!controller.state.is_loading, egui::Button::new("Sign In as Root")).clicked() {
                    controller.sign_in_root();
                }
                ui.label(RichText::new("Tip: Switch to root user for admin operations").small().color(Color32::GRAY));
            });
        });

        ui.separator();

        ui.vertical(|ui| {
            ui.label(RichText::new("Current User").strong());
            if controller.state.current_user.is_empty() {
                ui.label(RichText::new("Not signed in").color(Color32::GRAY));
            } else {
                ui.label(RichText::new(&controller.state.current_user).color(Color32::from_rgb(0, 200, 0)));
            }
        });
    });
}
