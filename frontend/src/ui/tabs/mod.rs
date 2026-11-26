pub mod account;
pub mod help;
pub mod home;
pub mod monitor;
pub mod settings;

use crate::app::Myapp;
use eframe::egui;

pub fn render_tab_content(app: &mut Myapp, ui: &mut egui::Ui) {
    match app.selected_tab {
        0 => home::render(app, ui),
        1 => monitor::render(app, ui),
        2 => account::render(app, ui),
        3 => settings::render(app, ui),
        4 => help::render(app, ui),
        _ => {
            app.selected_tab = 0;
            home::render(app, ui);
        }
    }
}
