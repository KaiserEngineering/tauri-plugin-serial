use std::io::{BufRead, BufReader};
use ts_rs::TS;

use tauri::{Runtime, Window};

use crate::state::SerialState;
use core::time;
use std::thread;
use tauri::State;
use tokio::time::timeout;

use serde::Serialize;

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub enum SerialErrors {
    Write,
    Read,
    Boot,
    Connection,
}

#[derive(Serialize, Debug, Clone, TS)]
#[ts(export)]
pub struct SerialPort {
    pub port_name: String,
    pub port_info: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SerialError {
    pub error_type: SerialErrors,
    pub message: String,
}

#[tauri::command]
pub async fn find_available_ports(
    serial_state: State<'_, SerialState>,
) -> Result<Vec<SerialPort>, SerialError> {
    let guard = serial_state.ports.lock().await;
    Ok(guard.to_owned())
}

pub fn read_serial(
    connection: &mut Box<dyn serialport::SerialPort>,
) -> Result<String, SerialError> {
    let mut buf = vec![];
    let mut b_reader = BufReader::with_capacity(1, connection);

    if let Err(error) = b_reader.read_until(0x0A, &mut buf) {
        eprintln!("Reading error: {error:?}");
        return Err(SerialError {
            error_type: SerialErrors::Read,
            message: error.to_string(),
        });
    }
    let mut output = std::str::from_utf8(&buf).unwrap().to_string();

    // Strip new line endings
    output = output.replace('\n', "");

    if output == "ERROR" || output == "nok" {
        println!("Failed to read/write: {output:?}");

        return Err(SerialError {
            error_type: SerialErrors::Read,
            message: output,
        });
    }

    println!("Successfully read from serial {output:?}");
    Ok(output)
}

// Write our data via Serial
pub async fn write_serial(
    connection: &mut Box<dyn serialport::SerialPort>,
    content: String,
) -> Result<String, SerialError> {
    match connection.write(content.as_bytes()) {
        Ok(write) => {
            if let Err(error) = connection.flush() {
                Err(SerialError {
                    error_type: SerialErrors::Write,
                    message: error.to_string(),
                })
            } else if write as u32 == content.len() as u32 {
                let content = content.replace('\n', "");
                println!("Successfully sent write to serial: {content}");

                match read_serial(connection) {
                    Err(e) => Err(e),
                    Ok(res) => Ok(res),
                }
            } else {
                Err(SerialError {
                    error_type: SerialErrors::Write,
                    message: format!(
                        "Incomplete write only wrote {} bytes of {}",
                        write,
                        content.len()
                    ),
                })
            }
        }
        Err(error) => Err(SerialError {
            error_type: SerialErrors::Write,
            message: error.to_string(),
        }),
    }
}

#[tauri::command]
pub async fn get_connection(serial_state: State<'_, SerialState>) -> Result<String, String> {
    //! Returns the current connecion name for our Tauri state
    match timeout(
        time::Duration::from_millis(10),
        serial_state.connection.lock(),
    )
    .await
    {
        Ok(lock) => match &*lock {
            Some(port) => Ok(port.name().unwrap()),
            None => {
                // std::mem::drop(serial_state.connection);
                return Err("Timed-out getting connection, dropping".to_string());
            }
        },
        Err(_) => Err("Timeout: no response in 10 milliseconds.".to_string()),
    }
}

#[tauri::command]
pub async fn drop_connection<R: Runtime>(
    serial_state: State<'_, SerialState>,
    window: Window<R>,
) -> Result<String, SerialError> {
    serial_state.connection.lock().await.take();
    if let Err(e) = window.emit("DISCONNECTED", {}) {
        return Err(SerialError {
            error_type: SerialErrors::Connection,
            message: format!("Failed to emit DISCONNECTED event: {e:?}"),
        });
    }
    Ok("Dropped connection".to_string())
}

#[tauri::command]
pub async fn connect<R: Runtime>(
    port_name: String,
    serial_state: State<'_, SerialState>,
    window: Window<R>,
) -> Result<String, String> {
    //! Connect to selected serial port based on port name
    println!("Model::Controller::connect called for {port_name}");
    let serial_port = serialport::new(&port_name, serial_state.baud_rate)
        .timeout(time::Duration::from_millis(500))
        .open();

    let mut guard = serial_state.connection.lock().await;

    match serial_port {
        Err(err) => {
            println!("Could not open port '{port_name}': {err}");
            Err(format!("Couldn't open serial port: {err}"))
        }
        Ok(active_connection) => {
            println!("New port connection opened");

            *serial_state.port.lock().await = port_name.to_string();
            *guard = Some(active_connection);

            if let Err(e) = guard.as_mut().unwrap().write_data_terminal_ready(true) {
                return Err(e.to_string());
            }
            println!("DTR signal written");

            // Sleep while the device reboots
            thread::sleep(time::Duration::from_millis(500));

            if let Err(e) = window.emit("CONNECTED", {}) {
                return Err(format!("Failed to emit CONNECTED event: {e:?}"));
            }

            Ok("New connection established".to_string())
        }
    }
}

#[tauri::command]
pub async fn write(
    serial_state: State<'_, SerialState>,
    content: String,
) -> Result<String, SerialError> {
    let mut guard = serial_state.connection.lock().await;

    if !guard.is_some() {
        return Err(SerialError {
            error_type: SerialErrors::Write,
            message: "Connection no longer valid, try reconnecting!".to_string(),
        });
    }

    return match &mut *guard {
        Some(port) => write_serial(port, content).await,
        None => Err(SerialError {
            error_type: SerialErrors::Write,
            message: "Could not lock Mutex for writing".into(),
        }),
    };
}

pub async fn send_dtr(
    conn: &mut Box<dyn serialport::SerialPort>,
    level: bool,
) -> Result<String, SerialError> {
    // Sent DTR signal
    if let Err(e) = conn.write_data_terminal_ready(level) {
        return Err(SerialError {
            error_type: SerialErrors::Boot,
            message: format!("Ran into issue sending DTR signal {e:?}"),
        });
    }
    println!("Wrote DTR signal to level {level:?}");
    Ok("DTR signal successfully sent".into())
}

#[tauri::command]
pub async fn dtr(serial_state: State<'_, SerialState>, level: bool) -> Result<String, SerialError> {
    let mut guard = serial_state.connection.lock().await;

    match &mut *guard {
        Some(port) => send_dtr(port, level).await,
        None => Err(SerialError {
            error_type: SerialErrors::Write,
            message: "No connection found".into(),
        }),
    }
}
