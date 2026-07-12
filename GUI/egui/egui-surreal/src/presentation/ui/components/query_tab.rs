use crate::presentation::controllers::AppController;
use egui::{Color32,
           RichText};

pub fn show_query_tab(controller: &mut AppController, ui: &mut egui::Ui) {
    ui.heading("Raw Query Interface");
    ui.separator();

    ui.vertical(|ui| {
        ui.label(RichText::new("SurrealQL Query").strong());
        ui.text_edit_multiline(&mut controller.state.raw_query);

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !controller.state.is_loading && !controller.state.raw_query.trim().is_empty(),
                    egui::Button::new("Execute Query"),
                )
                .clicked()
            {
                let query = controller.state.raw_query.clone();
                controller.execute_query(query);
            }

            if ui.button("Clear").clicked() {
                controller.state.raw_query.clear();
                controller.state.query_result.clear();
            }
        });

        ui.add_space(10.0);
        ui.separator();

        ui.label(RichText::new("Query Result").strong());
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            if controller.state.query_result.is_empty() {
                ui.label(RichText::new("No query executed yet").color(Color32::GRAY));
            } else {
                ui.text_edit_multiline(&mut controller.state.query_result);
            }
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("Query Examples").strong());
            ui.label("• SELECT * FROM person;");
            ui.label("• CREATE person SET name = 'John Doe';");
            ui.label("• UPDATE person:abc SET name = 'Jane Doe';");
            ui.label("• DELETE person WHERE name = 'John';");
        });
    });
}
