use std::io::{BufRead, BufReader};
use ts_rs::TS;

use crate::state::{SerialConnection, SerialState};
use core::time;
use std::thread;
use tauri::State;

use serde::Serialize;
use serialport::SerialPortInfo;
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub enum SerialErrors {
    Write,
    Read,
    Boot,
}

#[derive(Serialize, Debug, Clone, TS)]
#[ts(export)]
pub struct SerialPort {
    port_name: String,
    port_info: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SerialError {
    pub error_type: SerialErrors,
    pub message: String,
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

/// A trait that abstracts over the function(s) you want to mock out in tests
pub trait SerialManager {
    fn available_ports(&self) -> Result<Vec<SerialPortInfo>, serialport::Error>;
}

/// A struct which implements the trait to call the real function.
pub struct RealSerialManager;

impl SerialManager for RealSerialManager {
    fn available_ports(&self) -> Result<Vec<SerialPortInfo>, serialport::Error> {
        serialport::available_ports()
    }
}

/// The function that is generic over the manager. Can be private if desired.
pub async fn find_available_manager_ports<M: SerialManager>(
    manager: M,
) -> Result<Vec<SerialPort>, SerialError> {
    // Return vec of all ports found on device
    match manager.available_ports() {
        Ok(ports) => {
            Ok(ports
                .iter()
                .map(|p| {
                    // Right now we only grab Port name
                    SerialPort {
                        port_name: p.port_name.clone(),
                        port_info: match &p.port_type {
                            serialport::SerialPortType::UsbPort(info) => {
                                info.product.clone().unwrap()
                            }
                            _ => "".to_string(),
                        },
                    }
                })
                .collect())
        }
        Err(error) => Err(SerialError {
            error_type: SerialErrors::Write,
            message: error.to_string(),
        }),
    }
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
pub async fn find_available_ports() -> Result<Vec<SerialPort>, SerialError> {
    //! Wrapper that calls find_available_manager_ports which will then call the
    //! code to find the available serial ports on the machine.
    find_available_manager_ports(RealSerialManager).await
}

#[tauri::command]
pub async fn get_connection(
    serial_connection: State<'_, SerialConnection>,
) -> Result<String, String> {
    //! Returns the current connecion for our Tauri state
    let lock = serial_connection.port.lock().await;
    match &*lock {
        Some(port) => Ok(port.name().unwrap()),
        None => Err("No port".to_string()),
    }
}

#[tauri::command]
pub async fn connect(
    port_name: String,
    serial_connection: State<'_, SerialConnection>,
    serial_state: State<'_, SerialState>,
) -> Result<String, String> {
    //! Connect to selected serial port based on port name
    println!("Model::Controller::connect called for {port_name}");

    let port_binding = serial_connection.clone();
    let mut port_binding = port_binding.port.lock().await;

    // If we have a valid connection for the port we are trying to connect to,
    // there is nothing to do.
    if !port_binding.is_none() && port_binding.as_ref().unwrap().name().unwrap() == port_name {
        println!("Found existing connection");
        return Ok("Found existing connection".to_string());
    }

    let serial_port = serialport::new(&port_name, 57600)
        .timeout(time::Duration::from_millis(500))
        .open();

    match serial_port {
        Err(err) => {
            println!("Could not open port '{port_name}': {err}");

            Err(format!("Couldn't open serial port: {err}"))
        }
        Ok(active_port) => {
            println!("New port connection opened");

            *serial_state.port_name.lock().await = port_name.to_string();
            *port_binding = Some(active_port);

            if let Err(e) = port_binding
                .as_mut()
                .unwrap()
                .write_data_terminal_ready(true)
            {
                return Err(e.to_string());
            }
            println!("DTR signal written");

            // Sleep while the device reboots
            let sleep = time::Duration::from_millis(200);
            thread::sleep(sleep);

            Ok("New connection established".to_string())
        }
    }
}

#[tauri::command]
pub async fn write(
    serial_state: State<'_, SerialState>,
    conn: State<'_, SerialConnection>,
    content: String,
) -> Result<String, SerialError> {
    //! Write string content to connected serial port.
    let conn_clone = conn.clone();

    if let Err(e) = SerialConnection::validate_connection(serial_state, conn_clone).await {
        return Err(SerialError {
            error_type: SerialErrors::Write,
            message: e,
        });
    }

    let port_binding = conn.clone();
    let mut port_conn = port_binding.port.lock().await;

    match port_conn.as_mut() {
        Some(port) => write_serial(port, content).await,
        None => Err(SerialError {
            error_type: SerialErrors::Write,
            message: "No connection found".into(),
        }),
    }
}

#[tauri::command]
pub async fn dtr(
    serial_connection: State<'_, SerialConnection>,
    level: bool,
) -> Result<String, SerialError> {
    let port_binding = serial_connection.clone();
    let mut port_conn = port_binding.port.lock().await;

    match port_conn.as_mut() {
        Some(port) => send_dtr(port, level).await,
        None => Err(SerialError {
            error_type: SerialErrors::Write,
            message: "No connection found".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::UsbPortInfo;

    /// A struct which implements the trait to return mock data without calling the "actual" implementation.
    struct MockSerialManager;

    impl SerialManager for MockSerialManager {
        fn available_ports(&self) -> Result<Vec<SerialPortInfo>, serialport::Error> {
            // Return mock data.
            Ok(vec![serialport::SerialPortInfo {
                port_type: serialport::SerialPortType::UsbPort(UsbPortInfo {
                    vid: 1,
                    pid: 2,
                    serial_number: Some("serial_number".into()),
                    manufacturer: Some("kaiserengineering".into()),
                    product: Some("SHIFTLIGHT".into()),
                }),
                port_name: "Dog".to_string(),
            }])
        }
    }

    #[test]
    fn test_find_ports() {
        tauri::async_runtime::block_on(async move {
            let ports_found = find_available_manager_ports(MockSerialManager).await;

            assert_eq!(1, ports_found.unwrap().len());
        });
    }
}
