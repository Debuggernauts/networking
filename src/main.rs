use b15r::DdrPin::DDRA;
use b15r::PinPin::PINA;
use b15r::PortPin::PORTA;
use b15r::B15F;
use serialport::{ClearBuffer, SerialPort};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, io, io::Read, thread, thread::sleep, time::Duration};
use indicatif::{ProgressBar, ProgressStyle};
use v7::{controls, protocol::*, utilities::*};

// Arduino
const PORT_NAME: &str = "/dev/ttyUSB0";
const BAUD_RATE: u32 = 115200;
// const SEND_DELAY: Duration = Duration::from_millis(15);
const SEND_DELAY: Duration = Duration::from_millis(1000);

fn send_nano(port: &Arc<Mutex<Box<dyn SerialPort>>>, data: Vec<u8>) {
    let mut local_port = port.lock().unwrap();

    dbg!(&data);

    for byte in data {
        dbg!(byte);
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

/// Send all data until done
/// simultaneously recieve data
/// check if recieved data is correct
/// if not send enquiry for broken packets
/// if done sending, switch to recieving and answering enquiries
#[cfg(feature = "arduino")]
fn start(buffer: Vec<u8>) {
    let port = Arc::new(Mutex::new(
        serialport::new(PORT_NAME, BAUD_RATE)
            .timeout(Duration::from_millis(100))
            .open()
            .unwrap(),
    ));
    let mut state_transmission: Arc<Mutex<Transmission>>;
    let send_thread = std::thread::spawn({
        let port = Arc::clone(&port);
        if buffer.is_empty() {
            return;
        }
        let chunks = chunk_data(buffer, 128);
        let mut transmission = Transmission::new(make_transmission(chunks), false);
        state_transmission.lock().unwrap().clone_from(&transmission);
        let mut out_buffer = transmission.to_binary();
        /// for enquiring broken packets
        let mut reserve_buffer = out_buffer.clone();
        move || {
            loop {
                if out_buffer.is_empty() {
                    break;
                }
                let mut local_port = port.lock().unwrap();
                // take the first byte and send it
                let byte = out_buffer.remove(0);
                let _ = local_port.write(&[byte]);
                sleep(SEND_DELAY);
            }
            return;
        }
    });
    let recieve_thread = std::thread::spawn({
        let byte_map: HashMap<u8, &str> = HashMap::from([
            (controls::SOT, "SOT"),
            (controls::EOT, "EOT"),
            (controls::SOH, "SOH"),
            (controls::SOTX, "SOTX"),
            (controls::EOTX, "EOTX"),
            (controls::ENQ, "ENQ"),
            (controls::ACK, "ACK"),
            (controls::NAC, "NAC"),
        ]);
        let port = Arc::clone(&port);
        move || {
            loop {
                let mut local_port = port.lock().unwrap();
                let mut buffer: Vec<u8> = vec![0; 1];
                match local_port.read(&mut buffer) {
                    Ok(bytes_read) => {
                        if bytes_read != 0 {
                            // Data received, process it
                            let received_data = &buffer[..bytes_read];
                            buffer.push(received_data[0]);
                        } else {
                            dbg!(buffer);
                            // No data received, parse transmission
                            // let transmission = Transmission::from_bytes(buffer, byte_map.clone()); //TODO: wait for max to finish decoding
                            // let mut state = state_transmission.lock().unwrap();
                            // if transmission.header.is_enquiry {
                            // make new transmission with broken packets
                            // let needed_packets = transmission.packets.iter().filter(|packet| {
                            //     let id = packet.id;
                            //     state.packets.iter().find(|packet| packet.header.id == id)
                            // });
                            // let mut new_transmission = Transmission::new(needed_packets, false);
                            // let mut out_buffer = new_transmission.to_binary();
                            // loop {
                            //     let mut local_port = port.lock().unwrap();
                            // take the first byte and send it
                            // let byte = out_buffer.remove(0);
                            // let _ = local_port.write(&[byte]);
                            // sleep(SEND_DELAY);
                            // }
                            // } else {
                            // Check if transmission is correct
                            //if transmission == correct {
                            // Transmission is correct, send ACK
                            // let mut local_port = port.lock().unwrap();
                            // let _ = local_port.write(&[controls::ACK]);
                            // sleep(SEND_DELAY);
                            // } else {
                            // Transmission is incorrect, send enquiry with broken packets
                            // let mut local_port = port.lock().unwrap();
                            // let needed_packets = todo!();
                            // let chunks = chunk_data(needed_packets, 128);
                            // let mut new_transmission = Transmission::new(make_transmission(chunks), true);
                            // let mut out_buffer = new_transmission.to_binary();
                            // loop {
                            //     let byte = out_buffer.remove(0);
                            //     let _ = local_port.write(&[byte]);
                            //     sleep(SEND_DELAY);
                            // }
                            // }
                            // }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
        }
    });
}

#[cfg(feature = "b15f")]
fn start() {
    let recieve_thread = std::thread::spawn(|| loop {
        todo!("Read from board, check recieved and send to stdout, after being done immediately switch to sending to send enquiries for brocken packets");
    });
    recieve_thread.join().unwrap();
    // switch to checking & sending here
    todo!();
}

/// 15
const CLK_DELAY: u64 = 5;

fn main() {
    /*let mut stdin = io::stdin();
    let mut buffer = Vec::new();
    stdin.read_to_end(&mut buffer).unwrap();
    start(buffer);*/
    // let port = serialport::new(PORT_NAME, BAUD_RATE)
    // .timeout(Duration::from_millis(100))
    // .open()
    // .unwrap();
    // port.clear(ClearBuffer::Input).unwrap();
    // println!("Serial port opened at {}", PORT_NAME);
    // let port = Arc::new(Mutex::new(port));
    // std::thread::sleep(Duration::from_secs(3));
    // send_nano(
    // &port,
    // vec![
    // 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
    // 0x0F, 0x0E, 0x0D, 0x0C, 0x0B, 0x0A, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02,
    // 0x01,
    // ],
    // );
    let mut drv = B15F::get_instance();
    drv.set_register(DDRA, 0x0F); // set last 4 pins as output
    drv.set_register(PORTA, 0x0F); // set all pins to low
    let message: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 100, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let chunked = chunk_data(message, 10);
    // let data = read_stdin_as_vec_u8().unwrap();
    /*println!("{:?}", data.len());
    println!("{:?}", Transmission::new(make_transmission(chunk_data(data.clone()))).to_binary());
    println!("{:?}", Transmission::new(make_transmission(chunk_data(data))).to_binary().len());
    */
    let transmission_bins = dbg!(Transmission::new(make_transmission(chunked), false)).to_binary();
    // let transmission_bins = Transmission::new(make_transmission(chunked), false).to_binary();
    let pb = ProgressBar::new(transmission_bins.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("[{wide_bar}] [{percent}%] [{elapsed}] [ETA: {eta}] [{bytes_per_sec}] [{pos}/{len}]").unwrap()
        .progress_chars("=>-"));
    
    assert_eq!(transmission_bins[0], 9);
    dbg!(&transmission_bins);
    for byte in &transmission_bins {
        // println!("[{:2?}] {:04b}", byte >> 4, byte >> 4);
        drv.set_register(PORTA, ((byte & 0b01110000) | 0b10000000) >> 4);
        sleep(Duration::from_millis(CLK_DELAY));
        drv.set_register(PORTA, (byte & 0xF0) >> 4);
        sleep(Duration::from_millis(CLK_DELAY));

        // println!("[{:2?}] {:04b}", byte & 0xF, byte & 0xF);
        drv.set_register(PORTA, byte & 0b111);
        sleep(Duration::from_millis(CLK_DELAY));
        drv.set_register(PORTA, byte & 0xF);
        sleep(Duration::from_millis(CLK_DELAY));
        pb.inc(1);
    }
    drv.set_register(PORTA, 0x00);
}

// vec of u8 in transmission, each time something is added, try to parse