use serialport::{ClearBuffer, SerialPort};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::{io, thread, time::Duration};

const PORT_NAME: &str = "/dev/ttyUSB1";
const BAUD_RATE: u32 = 115200;
const SEND_DELAY: Duration = Duration::from_millis(15);

fn send_nano(port: &Arc<Mutex<Box<dyn SerialPort>>>, data: Vec<u8>) {
    let mut local_port = port.lock().unwrap();

    for byte in data {
        let _ = local_port.write(&[byte]);
        thread::sleep(SEND_DELAY);
    }
}
fn receive_nano(
    port: &Arc<Mutex<Box<dyn SerialPort>>>,
    buffer_size: usize,
) -> Result<Vec<u8>, io::Error> {
    let mut local_port = port.lock().unwrap();
    let mut buffer: Vec<u8> = vec![0; buffer_size];
    match local_port.read(&mut buffer) {
        Ok(bytes_read) => {
            let received_data = &buffer[..bytes_read];
            Ok(received_data.iter().map(|&byte| byte).collect())
        }
        Err(e) => Err(e),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open the serial port
    let port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(100))
        .open()?;
    port.clear(ClearBuffer::Input)?;
    println!("Serial port opened at {}", PORT_NAME);
    let port = Arc::new(Mutex::new(port));
    let send_thread = thread::spawn({
        let port = Arc::clone(&port);
        move || {
            loop {
                // Write data
                let bytes: Vec<u8> = String::from("gd").into_bytes();
                // TODO: Max [daten zum senden vorbereiten]
                println!("Send: {}", String::from_utf8_lossy(&bytes));
                send_nano(&port, bytes); // WICHTIG: nur ein byte at the time [sonnst kann man nicht gleichzeitig empfangen]
            }
        }
    });
    let receive_thread = thread::spawn({
        let port = Arc::clone(&port);
        move || {
            loop {
                match receive_nano(&port, 1) {
                    Ok(data) => {
                        print!("Received:{:2?} - [", data);
                        for byte in data {
                            print!("{:08b}, ", byte);
                        }
                        println!("]");
                    }
                    Err(e) => (), //eprintln!("Error receiving data: {}", e),
                }
                // TODO: Max [daten wieder zurückverwandeln]
            }
        }
    });
    send_thread.join().unwrap();
    receive_thread.join().unwrap();
    Ok(())
}
