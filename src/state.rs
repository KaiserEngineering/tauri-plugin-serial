use serde::Serialize;
use tokio::sync::Mutex;

use tauri::State;

use crate::command::connect;

#[derive(Default)]
pub struct SerialState {
    pub port_name: Mutex<String>,
}

#[derive(Default)]
pub struct SerialConnection {
    pub port: Mutex<Option<Box<dyn serialport::SerialPort>>>,
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

impl SerialConnection {
    pub async fn validate_connection(
        session: State<'_, SerialState>,
        port: State<'_, SerialConnection>,
    ) -> Result<String, String> {
        match port.port.try_lock() {
            Ok(_) => Ok("Old session is good".to_string()),
            _ => {
                let session_copy = session.clone();
                let port_name = session_copy.port_name.lock().await;

                connect(port_name.to_string(), port.clone(), session).await?;

                Ok("New session is good".to_string())
            }
        }
    }
}
