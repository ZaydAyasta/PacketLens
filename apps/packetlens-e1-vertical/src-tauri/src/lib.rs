//! Bridge experimental en proceso: Vue recibe un DTO ya proyectado y no conoce
//! los tipos del motor ni ejecuta reglas de ARP/CAM.

mod commands;

pub use commands::{contract_version, run_e1_s2_cold};

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::contract_version,
            commands::run_e1_s2_cold
        ])
        .run(tauri::generate_context!())
        .expect("error while running the PacketLens E1 vertical application");
}
