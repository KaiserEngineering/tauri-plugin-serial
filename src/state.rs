use serde::Serialize;
use tokio::sync::Mutex;

use tauri::State;

use crate::command::connect;

#[derive(Default)]
pub struct SerialState {
    pub port: Mutex<String>,
    pub connection: Mutex<Option<Box<dyn serialport::SerialPort>>>,
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
    pub async fn validate_connection(
        serial_state: State<'_, SerialState>,
    ) -> Result<String, String> {
        // Wait for our connection
        let serial_connection = serial_state.connection.lock().await;

        if !serial_connection.is_some() {
            let state_copy = serial_state.clone();
            let port_name = state_copy.port.lock().await.clone();

            connect(port_name.to_string(), state_copy).await?;

            return Ok("New session is good".to_string());
        }

        Ok("Old session is good".to_string())
    }
}
