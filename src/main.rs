use std::{io, time::Duration};
use std::fs::File;
use std::io::Write;
use std::mem;

use ansi_term::Color::Yellow;
use b15r::{B15F, Port0};
use b15r::DdrPin::DDRA;
use b15r::PinPin::PINA;
use b15r::PortPin::PORTA;
use indicatif::{ProgressBar, ProgressStyle};
use reed_solomon::Decoder;
use serialport::{new, ClearBuffer, SerialPort};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use v7::info;
use v7::protocol::{ProtocolDecoder, Transmission};
use v7::utilities::{chunk_data, make_transmission, print_colored_byte, read_stdin_as_vec_u8, slice_data, start_and_end, u16_to_u8_vec};

// todo: enquireys

#[allow(dead_code)]
const PORT_NAME: &str = "/dev/ttyUSB0";
#[allow(dead_code)]
const BAUD_RATE: u32 = 115200;
// const CLK_DELAY: u64 = 4;
const CLK_DELAY: u128 = 100;

const CHUNK_SIZE: usize = 32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut transmission_packet_array: Vec<Vec<u8>> = (0..u16::MAX)
        .map(|_| Vec::new())
        .collect();

    ////////// init //////////
    let mut clock = 0;

    // let mut drv = setup_b15();
    let mut port = setup_nano();

    ////////// data setup //////////
    // from file -> Transmission
    let message: Vec<u8> = vec![
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 100, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7,
        8, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    ];
    // let data = read_stdin_as_vec_u8().unwrap();

    let chunked = chunk_data(message, CHUNK_SIZE);

    let transmission = Transmission::new(make_transmission(chunked), false);
    let mut transmission_bins = transmission.clone().to_binary();
    println!("trans bins: {:?}", transmission_bins);

    transmission_bins = ready_for_send(transmission_bins);

    println!("trans bins: {:?}", transmission_bins);

    for _ in 0..20 {
        transmission_bins.insert(0, 0);
        transmission_bins.insert(0, 0b1000);
    }
    let pb = ProgressBar::new(transmission_bins.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("[{wide_bar}] [{percent}%] [{elapsed}] [ETA: {eta}] [{bytes_per_sec}] [{pos}/{len}]").unwrap()
        .progress_chars("=>-"));

    ////////// main loop //////////
    let mut received: Vec<u8> = Vec::new();


    // zum Testen
    read_stdin_as_vec_u8().expect("dumm");

    let mut previousMillis = SystemTime::now().duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();


    loop {
        ////////// send //////////
        let currentMillis = SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        // Check if the interval has elapsed
        if currentMillis - previousMillis >= CLK_DELAY {
            // Save the current time as the last executed time
            previousMillis = currentMillis;
            if !transmission_bins.is_empty() {
                let byte = transmission_bins.remove(0);
                // send_b15(&mut drv, byte);
                send_nano(&mut port, byte);

                pb.inc(1);
            }
        }

        ////////// receive //////////

        let receiver: Result<u8, io::Error> = {
            // receive_b15(&mut drv, &mut clock)
            receive_nano(&mut port, 1)
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
                    let start_time = Instant::now(); // Record the start time

                    let false_ids = auswertung(data, start, end, &mut transmission_packet_array, &transmission);

                    let duration = start_time.elapsed();
                    for _ in 0..10 {
                        println!("Function took {:?} to execute", duration);
                    }
                    
                    if false_ids.is_empty() {
                        //TODO: in datei schreiben
                    } else {
                        let chunked = chunk_data(u16_to_u8_vec(false_ids), CHUNK_SIZE);
                        let transmission = Transmission::new(make_transmission(chunked), true);
                        transmission_bins.extend(ready_for_send(transmission.clone().to_binary()));
                    }
                }
                println!("]");
            }
            Err(_e) => (),
        }
    }
}


fn ready_for_send(transmission_bins: Vec<u8>) -> Vec<u8> {
    let new_transmission_bins: Vec<u8> = transmission_bins
        .iter()
        .flat_map(|&byte| {
            // Oberes und unteres Nibble berechnen
            // println!("byte: {:08b}", byte);
            let upper_nibble = byte >> 4;
            let upper_clock = upper_nibble & 0b1000;
            let lower_nibble = byte & 0xF;
            let lower_clock = lower_nibble & 0b1000;

            // println!("upper: {:04b}", upper_nibble | (!upper_clock & 0b1000));
            // println!("upclk: {:04b}", upper_nibble);
            // println!("lower: {:04b}", lower_nibble & ((!lower_clock & 0b1000) | 0b0111));
            // println!("lwclk: {:04b}", lower_nibble);

            vec![
                // Daten für oberes Nibble
                upper_nibble | (!upper_clock & 0b1000),
                // Clock-Signal für oberes Nibble
                upper_nibble,
                // Daten für unteres Nibble
                lower_nibble & ((!lower_clock & 0b1000) | 0b0111),
                // Clock-Signal für unteres Nibble
                lower_nibble,
            ]
        })
        .collect();
    new_transmission_bins
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
    print!("Send: [");
    print_colored_byte(byte & 0xF);
    println!("]");
    port.write_all(&[byte & 0xF]).expect("port write panicked");
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
    print!("Send: [");
    print_colored_byte(byte & 0xF);
    println!("]");
    // drv.set_register(PORTA, byte & 0xF);
    drv.digital_write(Port0, byte & 0xF);
}

#[allow(dead_code)]
fn receive_b15(drv: &mut B15F, clock: &mut u8) -> Result<u8, io::Error> {
    // let received_data = (drv.get_register(PINA) & 0xF0) >> 4;
    let received_data = drv.digital_read(Port0) & 0xF;
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
fn auswertung(data: Vec<u8>, start: usize, end: usize, transmission_packet_array: &mut Vec<Vec<u8>>, init_transmission: &Transmission) -> Vec<u16> {
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

    if transmission.header.is_enquiry {
        let mut ids: Vec<u16> = Vec::new();
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
                    buffer.data().to_vec()[header_vec.len()..].clone_into(&mut packet.data);
                    packet.ecc = buffer.ecc().to_vec();
                    let local_ids: Vec<u16> = packet.data.chunks(2).map(|x| {
                        u16::from_le_bytes([x[0], x[1]])
                    }).collect();
                    ids.extend(local_ids);
                    
                    // TODO: respond with data for requested packets
                    // init_transmission
                    //let chunked = chunk_data(u16_to_u8_vec(false_ids), CHUNK_SIZE);
                    //let transmission = Transmission::new(make_transmission(chunked), true);
                    //transmission_bins.extend(ready_for_send(transmission.clone().to_binary()));
                }
                Err(e) => {
                    // TODO: packet not recoverable, enquiry
                    info!("Packet unrecoverable: {e:?}\n{packet:?}, We're fucked!");
                }
            }
        }
        todo!("Figure out enquiries heh")
    } else {
    // todo: TransHeader überprüfen ob alle packet da und so
    //let mut file_data: Vec<u8> = Vec::new();
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
                buffer.data().to_vec()[header_vec.len()..].clone_into(&mut packet.data);
                packet.ecc = buffer.ecc().to_vec();
                println!(
                    "{}",
                    Yellow.paint("Alles gut!!! -> in datei schreiben (todo)")
                );
                // todo: eigentlich. in Vec schreiben und erst wenn alle da sind: in file
                // file_data[packet.id] = packet.data
                //file_data.extend(packet.data.clone());
                transmission_packet_array[packet.header.id as usize] = packet.data;
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
    //let mut file = File::create("output.bin").unwrap();
    //file.write_all(&file_data).expect("file write panicked");
    unrepairable_packets
    // panic!("Do you have panic?? ;)");
    }
}
