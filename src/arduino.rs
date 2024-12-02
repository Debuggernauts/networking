use std::{io, thread, time::Duration};
use std::io::Write;

use ansi_term::Colour;
use serialport::{ClearBuffer, SerialPort};

use crate::protocol::ProtocolDecoder;
use crate::utilities::nibbles_to_bytes;
/*
use b15r::B15F;
use b15r::Port0;
use b15r::DdrPin::DDRA;
use b15r::PortPin::PORTA;
use b15r::PinPin::PINA;
*/

const is_b15: bool = false;

const PORT_NAME: &str = "/dev/ttyUSB0";
const BAUD_RATE: u32 = 115200;
const SEND_DELAY: Duration = Duration::from_millis(40);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open the serial port
    let mut port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(100))
        .open()?;

    port.clear(ClearBuffer::Input)?;
    port.clear(ClearBuffer::Output)?;

    thread::sleep(SEND_DELAY);
    println!("Serial port opened at {}", PORT_NAME);
    let _ = port.write(&[0xF0, 0b1, 0xF0, 0b1]); // random bytes lol

    let mut bytes: Vec<u8> = String::from("H").into_bytes();
    let mut received: Vec<u8> = Vec::new();
    loop {
        // Write data

        // TODO: Max [daten zum senden vorbereiten]
        if !bytes.is_empty() {
            println!("Send: {}", String::from_utf8_lossy(&bytes));
            send_nano(&mut port, bytes.remove(0)); // WICHTIG: nur ein byte at the time [sonst kann man nicht gleichzeitig empfangen]
        }

        match receive_nano(&mut port, 1) {
            Ok(data) => {
                print!("Received:{:2?} - [", data.as_bytes());
                for byte in data.as_bytes() {
                    print_colored_byte(*byte);
                    received.push(*byte);
                    if received.len() < 6 {
                        continue
                    }
                    let range = start_and_end(&received);
                    match range {
                        Some((start, end)) => {
                            println!("{:?}", received);
                            let data = received.clone();
                            thread::spawn(move || {
                                let sliced_data = slice_data(data, start, end);
                                println!("sliced_data: {:?}", sliced_data);
                                let mut squashed_data: Vec<u8> = Vec::new();
                                for x in sliced_data.chunks(2) {
                                    let mut res: u8 = 0b0;
                                    res |= x[0] << 4;
                                    res |= x[1];
                                    squashed_data.push(res);
                                }
                                println!("squashed_data: {:?}", squashed_data);
                                let mut p = ProtocolDecoder::new(squashed_data);
                                p.decode();
                            });
                        }
                        None => (),
                    }
                }
                println!("]");
            }
            Err(e) => (),
        }
    }
}

fn slice_data(received: Vec<u8>, start: usize, end: usize) -> Vec<u8> {
    received.get(start..end + 3).expect("Failed to slice recieved data").to_vec()
}

/// checks if the data has a start and end
fn start_and_end(p0: &Vec<u8>) -> Option<(usize, usize)> {
    let mut start_found = false;
    let mut start_index = 0;
    let mut end_found = false;
    let mut end_index = 0;
    for i in 0..p0.len() - 2 {
        let nibble_0 = p0[i];
        let nibble_1 = p0[i + 1];
        let nibble_2 = p0[i + 2];
        // there are 2 theoretically possible combinations of SOT (main difference being clock)
        if ((nibble_0 == 0b0 || nibble_0 == 0b1000) && (nibble_1 == 0b0 || nibble_1 == 0b1001) && (nibble_2 == 0b111 || nibble_2 == 0b1111)) {
            // found SOT
            start_found = true;
            start_index = i;
        }
        if ((nibble_0 == 0b0 || nibble_0 == 0b1000) && (nibble_1 == 0b1001 || nibble_1 == 0b1) && (nibble_2 == 0b1 || nibble_2 == 0b1001)) {
            // found EOT
            end_found = true;
            end_index = i;
            break;
        }
    }
    if start_found && end_found {
        Some((start_index, end_index))
    } else {
        None
    }
}


fn print_colored_byte(byte: u8) {
    let bits: Vec<String> = (0..4)
        .rev()
        .map(|i| {
            // Get the value of the bit
            let bit = (byte >> i) & 1;

            // Colorize the second bit (bit index 6)
            if i == 3 {
                if bit == 1 {
                    Colour::Green.paint(format!("{}", bit)).to_string()
                } else {
                    Colour::Red.paint(format!("{}", bit)).to_string()
                }
            } else {
                format!("{}", bit)
            }
        })
        .collect();

    // Join the colored bits into a string
    print!("{},", bits.join(""));
}

fn send_nano(port: &mut Box<dyn SerialPort>, data: u8) {
    println!("s: {:08b}", data & 0xF0);
    let _ = port.write(&[data & 0xF0]);
    thread::sleep(SEND_DELAY);
}

fn receive_nano(port: &mut Box<dyn SerialPort>, buffer_size: usize) -> Result<String, io::Error> {
    let mut buffer: Vec<u8> = vec![0; buffer_size];
    match port.read(&mut buffer) {
        Ok(bytes_read) => {
            let received_data = &buffer[..bytes_read];
            Ok(String::from_utf8_lossy(received_data).to_string())
        }
        Err(e) => Err(e),
    }
}

/*
Probleme:
- Arduino ersten 2 bits nicht gesendet
- noice vor empfangen
- gleichzeitiges Senden Empfangen
- Errorkorrektur
- Fehler erkennen, anfragen nach Fehlern senden und beantworten
-

 */
