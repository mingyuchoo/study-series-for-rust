use crate::presentation::controllers::AppController;
use egui::{Color32,
           RichText};

pub fn show_people_tab(controller: &mut AppController, ui: &mut egui::Ui) {
    ui.heading("People Management");
    ui.separator();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.label(RichText::new("Create New Person").strong());
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut controller.state.person_name);
                });

                let create_enabled = !controller.state.person_name.trim().is_empty() && !controller.state.is_loading;
                if ui.add_enabled(create_enabled, egui::Button::new("+ Create Person")).clicked() {
                    let name = controller.state.person_name.trim().to_string();
                    controller.create_person(name);
                    controller.state.person_name.clear();
                }
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(RichText::new("Delete Person").strong());
                ui.horizontal(|ui| {
                    ui.label("ID:");
                    ui.text_edit_singleline(&mut controller.state.person_id_to_delete);
                });
                ui.label(RichText::new("Tip: Leave empty to delete all people").small().color(Color32::GRAY));

                if ui.add_enabled(!controller.state.is_loading, egui::Button::new("Delete")).clicked() {
                    let id = controller.state.person_id_to_delete.clone();
                    controller.delete_person(id);
                }
            });

            ui.add_space(10.0);

            if ui.add_enabled(!controller.state.is_loading, egui::Button::new("List All People")).clicked() {
                controller.list_people();
            }
        });

        ui.separator();

        ui.vertical(|ui| {
            ui.label(RichText::new("People List").strong());
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                if controller.state.people_list.is_empty() {
                    ui.label(RichText::new("No people loaded. Click 'List All People' to refresh.").color(Color32::GRAY));
                } else {
                    ui.text_edit_multiline(&mut controller.state.people_list);
                }
            });
        });
    });
}
