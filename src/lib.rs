use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Manager, Runtime,
};

use state::SerialState;

use crate::command::{connect, dtr, find_available_ports, get_connection, write};

pub mod command;
pub mod state;

pub struct Builder {
    baud_rate: u32,
}

impl Default for Builder {
    fn default() -> Self {
        Self { baud_rate: 57600 }
    }
}

impl Builder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn option_a(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::new("serial")
            .invoke_handler(tauri::generate_handler![
                find_available_ports,
                get_connection,
                connect,
                write,
                dtr
            ])
            .setup(move |app_handle, api| {
                println!("Config is {:?}", api.config());
                app_handle.manage(SerialState {
                    port: Default::default(),
                    connection: Default::default(),
                    baud_rate: self.baud_rate,
                });
                Ok(())
            })
            .build()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}
