use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

use state::SerialConnection;

use crate::command::{connect, dtr, find_available_ports, get_connection, write};

mod command;
mod state;

#[tauri::command]
// this will be accessible with `invoke('plugin:serial|initialize')`.
fn initialize() {}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("serial")
        .invoke_handler(tauri::generate_handler![
            initialize,
            find_available_ports,
            get_connection,
            connect,
            write,
            dtr
        ])
        .setup(move |app_handle| {
            app_handle.manage(SerialConnection {
                port: Default::default(),
            });
            Ok(())
        })
        .build()
}
