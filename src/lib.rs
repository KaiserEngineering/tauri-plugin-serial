#![feature(async_closure)]
#![feature(try_trait_v2)]

use crate::command::{
    connect, drop_connection, dtr, find_available_ports, get_connection, write, SerialPort,
};
use serialport::SerialPortInfo;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime, State, Window,
};
use tokio::sync::Mutex;

pub mod command;
pub mod state;
use serde::Serialize;
use state::SerialState;

#[derive(Serialize, Debug, Clone)]
struct Payload {
    devices: Vec<command::SerialPort>,
}

// init a background process on the command, and emit periodic events only to the window that used the command
#[tauri::command]
fn watch_devices<R: Runtime>(serial_state: State<'_, SerialState>, window: Window<R>) {
    let state = Arc::clone(&serial_state.ports);
    std::thread::spawn(async move || loop {
        let mut known_devices = state.lock().await;
        let devices = serialport::available_ports();

        match devices {
            Ok(devices) => {
                let serial_ports = massage_devices_list(devices);
                // TODO: Do a real check
                if serial_ports.len() != known_devices.len() {
                    *known_devices = serial_ports;

                    if let Err(e) = window.emit(
                        "DEVICE_LIST_UPDATED",
                        Payload {
                            devices: known_devices.to_vec(),
                        },
                    ) {
                        eprintln!("Failed to emit DEVICE_LIST_UPDATED event: {e:?}");
                    }
                }
            }
            Err(err) => eprint!("Error getting available_ports: {err:?}"),
        };
        std::mem::drop(known_devices);
        thread::sleep(Duration::from_millis(200));
    });
}

fn massage_devices_list(devices: Vec<SerialPortInfo>) -> Vec<SerialPort> {
    devices
        .iter()
        .map(|p| {
            // Right now we only grab Port name
            SerialPort {
                port_name: p.port_name.clone(),
                port_info: match &p.port_type {
                    serialport::SerialPortType::UsbPort(info) => info.product.clone().unwrap(),
                    _ => "".to_string(),
                },
            }
        })
        .collect()
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("serial")
        .invoke_handler(tauri::generate_handler![
            get_connection,
            connect,
            write,
            dtr,
            drop_connection,
            find_available_ports,
            watch_devices
        ])
        .setup(move |app_handle, _api| {
            let baud_rate = app_handle
                .config()
                .plugins
                .0
                .get("plugin_serial")
                .unwrap()
                .get("baud_rate")
                .unwrap()
                .as_u64()
                .unwrap() as u32;

            let state = match serialport::available_ports() {
                Ok(ports_found) => SerialState {
                    port: Default::default(),
                    connection: Default::default(),
                    baud_rate,
                    ports: Arc::new(Mutex::new(massage_devices_list(ports_found))),
                },
                Err(err) => {
                    eprint!("Could not get initial available_ports {err:?}");
                    SerialState {
                        port: Default::default(),
                        connection: Default::default(),
                        baud_rate: 57600,
                        ports: Default::default(),
                    }
                }
            };

            app_handle.manage(state);
            Ok(())
        })
        .build()
}
