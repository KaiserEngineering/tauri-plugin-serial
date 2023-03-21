use rusb;
use serde::Serialize;
use tokio::sync::Mutex;

use tauri::State;

use crate::command::connect;

#[derive(Default)]
pub struct SerialState {
    pub port: Mutex<String>,
    pub connection: Mutex<Option<Box<dyn serialport::SerialPort>>>,
    pub baud_rate: u32,
}

#[derive(Serialize, Clone)]
pub struct InvokeResult {
    pub code: i32,
    pub message: String,
}

#[derive(Serialize, Clone)]
pub struct ReadData<'a> {
    pub data: &'a [u8],
    pub size: usize,
}

impl SerialState {
    pub fn usb_is_mounted(port_name: &str) -> bool {
        for device in rusb::devices().unwrap().iter() {
            let device_desc = device.device_descriptor().unwrap();

            println!("{:?}", device.address());

            if device.address().to_string() == port_name.to_string() {
                return true;
            }
        }
        false
    }

    pub async fn validate_connection(
        serial_state: State<'_, SerialState>,
    ) -> Result<String, String> {
        // Wait for our connection
        let serial_connection = serial_state.connection.lock().await;

        let state_copy = serial_state.clone();
        let port_name = state_copy.port.lock().await.clone();

        if !serial_connection.is_some() || !SerialState::usb_is_mounted(&port_name) {
            connect(port_name.to_string(), state_copy).await?;

            return Ok("New session is good".to_string());
        }

        Ok("Old session is good".to_string())
    }
}
