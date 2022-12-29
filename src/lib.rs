use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

use state::SerialConnection;

mod command;
mod state;

#[tauri::command]
// this will be accessible with `invoke('plugin:serial|initialize')`.
fn initialize() {}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("serial")
        .invoke_handler(tauri::generate_handler![initialize])
        .setup(move |app_handle| {
            app_handle.manage(SerialConnection {
                port: Default::default(),
            });
            Ok(())
        })
        .build()
}
