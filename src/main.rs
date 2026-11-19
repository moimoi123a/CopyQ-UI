#![forbid(unsafe_code)]

mod config;
mod copyq;
mod ui;

use gtk4 as gtk;
use gtk::glib;
use gtk::prelude::*;

const APP_ID: &str = "io.github.copyq_ui.Popup";

fn main() -> glib::ExitCode {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-V" | "--version" => {
                println!("copyq-ui {}", env!("CARGO_PKG_VERSION"));
                return glib::ExitCode::SUCCESS;
            }
            "-h" | "--help" => {
                println!(
                    "copyq-ui {} — Giao diện popup GTK4 siêu nhẹ cho CopyQ\n\n\
                     Cách dùng:\n\
                       copyq-ui             Bật/tắt cửa sổ popup (gắn vào phím Win+V)\n\
                       copyq-ui --version   Xem phiên bản\n\
                       copyq-ui --help      Xem hướng dẫn\n\n\
                     Cấu hình cá nhân:\n\
                       ~/.config/copyq-ui/config.conf\n\
                       ~/.config/copyq-ui/style.css",
                    env!("CARGO_PKG_VERSION")
                );
                return glib::ExitCode::SUCCESS;
            }
            _ => {}
        }
    }

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(ui::build_ui);
    app.run_with_args::<&str>(&[])
}
