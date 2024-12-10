use std::{io, thread, time::Duration};
use std::fs::File;
use std::io::Write;
use std::thread::sleep;

use ansi_term::Color::Yellow;
use b15r::B15F;
use b15r::DdrPin::DDRA;
use b15r::PinPin::PINA;
use b15r::PortPin::PORTA;
use indicatif::{ProgressBar, ProgressStyle};
use reed_solomon::Decoder;
use serialport::{ClearBuffer, SerialPort};

use v7::info;
use v7::protocol::{ProtocolDecoder, Transmission};
use v7::utilities::{chunk_data, make_transmission, print_colored_byte, read_stdin_as_vec_u8, slice_data, start_and_end, u16_to_u8_vec};

// todo: beide seiten senden, empfangen, encoden
// todo: enquireys

#[allow(dead_code)]
const PORT_NAME: &str = "/dev/ttyUSB0";
#[allow(dead_code)]
const BAUD_RATE: u32 = 115200;
// const CLK_DELAY: u64 = 4;
const CLK_DELAY: u64 = 15;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ////////// init //////////
    let mut clock = 0;

    let mut drv = setup_b15();
    // let mut port = setup_nano();

    // delay zum warten auf anderes Gerät
    // sleep(Duration::from_millis(2000));

    ////////// data setup //////////
    // from file -> Transmission
    let message: Vec<u8> = vec![
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 100, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7,
        8, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    ];
    // let data = read_stdin_as_vec_u8().unwrap();

    let chunked = chunk_data(message, 32);

    let transmission = Transmission::new(make_transmission(chunked), false);
    let mut transmission_bins = transmission.clone().to_binary();

    for _ in 0..30 {
        transmission_bins.insert(0, 0);
    }
    let pb = ProgressBar::new(transmission_bins.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("[{wide_bar}] [{percent}%] [{elapsed}] [ETA: {eta}] [{bytes_per_sec}] [{pos}/{len}]").unwrap()
        .progress_chars("=>-"));

    ////////// main loop //////////
    let mut received: Vec<u8> = Vec::new();


    // zum testen
    read_stdin_as_vec_u8().expect("dumm");

    loop {
        ////////// send //////////
        if !transmission_bins.is_empty() {
            let byte = transmission_bins.remove(0);
            // b15
            send_b15(&mut drv, byte);
            // nano
            // send_nano(&mut port, byte);

            pb.inc(1);
        }

        ////////// receive //////////

        let receiver: Result<u8, io::Error> = {
            receive_b15(&mut drv, &mut clock)
            // receive_nano(&mut port, 1)
        };

        match receiver {
            Ok(byte) => {
                print!("Received:{byte:2?} - [");
                print_colored_byte(byte);
                received.push(byte);
                if received.len() < 6 {
                    continue;
                }
                let range = start_and_end(&received);
                if let Some((start, end)) = range {
                    let data = received.clone();
                    received.clear();
                    // TODO: this returns ids of packets that are not recoverable
                    let false_ids = auswertung(data, start, end);
                    let chunked = chunk_data(u16_to_u8_vec(false_ids), 32);

                    let transmission = Transmission::new(make_transmission(chunked), true);
                    transmission_bins.extend(transmission.clone().to_binary());
                }
                println!("]");
            }
            Err(_e) => (),
        }
    }
}

////////// nano functions //////////
#[allow(dead_code)]
fn setup_nano() -> Box<dyn SerialPort> {
    let mut port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(100))
        .open()
        .unwrap();

    port.clear(ClearBuffer::Input)
        .expect("port input buffer clear panicked");
    port.clear(ClearBuffer::Output)
        .expect("port output buffer clear panicked");

    port.write_all(&[0xFF]).expect("port write panicked");

    println!("Serial port opened at {}", Yellow.paint(PORT_NAME));
    port
}

#[allow(dead_code)]
fn send_nano(port: &mut Box<dyn SerialPort>, byte: u8) {
    //println!("{:08b}", byte);
    print!("Send: [");
    print_colored_byte(byte >> 4);
    println!("]");
    //println!("[{:2?}] {:04b}", ((byte & 0b01110000) | 0b10000000) >> 4, ((byte & 0b01110000) | 0b10000000) >> 4);
    port.write_all(&[((byte & 0b0111_0000) | 0b1000_0000) >> 4])
        .expect("port write panicked");
    //println!("[{:2?}] {:04b}", (byte & 0xF0) >> 4, (byte & 0xF0) >> 4);
    sleep(Duration::from_millis(CLK_DELAY));
    port.write_all(&[(byte & 0xF0) >> 4])
        .expect("port write panicked");
    sleep(Duration::from_millis(CLK_DELAY));

    print!("send: [");
    print_colored_byte(byte & 0xF);
    println!("]");
    //println!("[{:2?}] {:04b}", byte & 0b111, byte & 0b111);
    port.write_all(&[byte & 0b111]).expect("port write panicked");
    //println!("[{:2?}] {:04b}", byte & 0xF, byte & 0xF);
    sleep(Duration::from_millis(CLK_DELAY));
    port.write_all(&[byte & 0xF]).expect("port write panicked");
    sleep(Duration::from_millis(CLK_DELAY));
}

#[allow(dead_code)]
fn receive_nano(port: &mut Box<dyn SerialPort>, buffer_size: usize) -> Result<u8, io::Error> {
    let mut buffer: Vec<u8> = vec![0; buffer_size];
    match port.read(&mut buffer) {
        Ok(bytes_read) => {
            let received_data = &buffer[..bytes_read];
            Ok(received_data[0])
        }
        Err(e) => Err(e),
    }
}

////////// b15 functions //////////
#[allow(dead_code)]
fn setup_b15() -> B15F {
    let mut drv = B15F::get_instance();
    drv.set_register(DDRA, 0x0F); // set last 4 pins as output
    drv.set_register(PORTA, 0x0F); // set all pins to low
    drv
}

#[allow(dead_code)]
fn send_b15(drv: &mut B15F, byte: u8) {
    dbg!();
    // println!("[{:2?}] {:04b}", byte >> 4, byte >> 4);
    print!("Send: [");
    print_colored_byte(byte >> 4);
    println!("]");
    drv.set_register(PORTA, ((byte & 0b0111_0000) | 0b1000_0000) >> 4);
    sleep(Duration::from_millis(CLK_DELAY));
    drv.set_register(PORTA, (byte & 0xF0) >> 4);
    sleep(Duration::from_millis(CLK_DELAY));

    // println!("[{:2?}] {:04b}", byte & 0xF, byte & 0xF);
    print!("Send: [");
    print_colored_byte(byte & 0xF);
    println!("]");
    drv.set_register(PORTA, byte & 0b111);
    sleep(Duration::from_millis(CLK_DELAY));
    drv.set_register(PORTA, byte & 0xF);
    sleep(Duration::from_millis(CLK_DELAY));
}

#[allow(dead_code)]
fn receive_b15(drv: &mut B15F, clock: &mut u8) -> Result<u8, io::Error> {
    let received_data = (drv.get_register(PINA) & 0xF0) >> 4;
    let new_clock = received_data & 0b1000;
    // dbg!();
    if *clock == new_clock {
        Err(io::Error::new(io::ErrorKind::Other, "Failed to read byte"))
    } else {
        *clock = new_clock;
        // print_colored_byte(received_data);
        Ok(received_data)
    }
}

////////// other functions //////////
fn auswertung(data: Vec<u8>, start: usize, end: usize) -> Vec<u16> {
    let decode_thread = thread::spawn(move || {
        let sliced_data = slice_data(data, start, end);
        let mut squashed_data: Vec<u8> = Vec::new();
        for x in sliced_data.chunks(2) {
            let mut res: u8 = 0b0;
            res |= x[0] << 4;
            let second_nibble = x.get(1);
            if let Some(nibble) = second_nibble {
                res |= nibble;
            }
            squashed_data.push(res);
        }
        let mut p = ProtocolDecoder::new(squashed_data);
        let transmission = p.decode();

        // todo: TransHeader überprüfen ob alle packet da und so
        let mut file_data: Vec<u8> = Vec::new();
        let mut unrepairable_packets: Vec<u16> = Vec::new();
        for mut packet in transmission.packets {
            let decoder = Decoder::new(packet.header.ecc_size as usize);
            let header_vec = packet.header.to_vec();
            let mut msg = header_vec.clone();
            msg.append(&mut packet.data.clone());
            msg.append(&mut packet.ecc.clone());
            println!("{:?}", &msg);
            let decoded = decoder.correct_err_count(&msg, None);
            match decoded {
                Ok(content) => {
                    let buffer = content.0;
                    let errors = content.1;
                    if errors > 0 {
                        eprintln!("Packet {} had {} errors!", packet.header.id, errors);
                    }
                    packet.data = buffer.data().to_vec()[header_vec.len()..].to_owned();
                    packet.ecc = buffer.ecc().to_vec();
                    println!(
                        "{}",
                        Yellow.paint("Alles gut!!! -> in datei schreiben (todo)")
                    );
                    // todo: eigentlich. in Vec schreiben und erst wenn alle da sind: in file
                    // file_data[packet.id] = packet.data
                    file_data.extend(packet.data);
                }
                Err(e) => {
                    // TODO: packet not recoverable, enquiry
                    // wenn ein packet komplett fehlt?
                    // enquiry erstellen, in send schicken
                    info!("Packet unrecoverable: {e:?}\n{packet:?}");
                    unrepairable_packets.push(packet.header.id);
                }
            }
        }
        let mut file =
            File::create(format!("output{}.bin", chrono::Utc::now().timestamp())).unwrap();
        file.write_all(&file_data).expect("file write panicked");
        unrepairable_packets
        // panic!("Do you have panic?? ;)");
    });
    decode_thread.join().unwrap()
}
