mod common;
mod ui;
mod widget;

use crate::ui::IcedApp;
use iced::Task;
use parakesh_common::PKAppAsync;

#[tokio::main]
async fn main() {
    println!("Parakesh UI Iced");

    let backend = PKAppAsync::new().expect("Backend creation error");

    let _res = iced::application("Parakesh", IcedApp::update, IcedApp::view)
        .subscription(IcedApp::subscription)
        .run_with(|| (IcedApp::new(backend), Task::none()));
}
