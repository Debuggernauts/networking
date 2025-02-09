use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{io, time::Duration};

use ansi_term::Color::Yellow;
use b15r::DdrPin::DDRA;
use b15r::PortPin::PORTA;
use b15r::{Port0, B15F};
use indicatif::{ProgressBar, ProgressStyle};
use reed_solomon::Decoder;
use serialport::{ClearBuffer, SerialPort};

use v7::protocol::{ProtocolDecoder, State, Transmission};
use v7::utilities::{
    chunk_data, make_transmission, print_colored_byte, read_stdin_as_vec_u8, ready_for_send,
    slice_data, start_and_end, u16_to_u8_vec,
};
use v7::{error, info};

// TODO: 1 Packet pro Transmission

#[allow(dead_code)]
const PORT_NAME: &str = "/dev/ttyUSB0";
#[allow(dead_code)]
const BAUD_RATE: u32 = 115_200;
const TIMEOUT: u128 = 2000;

// Nano <-> Nano: 4ms
// B15 <-> Nano: 29ms (15ms?)
const CLK_DELAY: u128 = 4;

const CHUNK_SIZE: usize = 48;

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ////////// init //////////
    let mut state = State::Normal;
    let mut transmission_packet_array: Vec<Vec<u8>> = (0..u16::MAX).map(|_| Vec::new()).collect();
    let mut broken_ids: Vec<u16> = Vec::new();

    #[allow(dead_code)]
    let mut clock = 0; // needed for b15
                       // let mut drv = setup_b15();
    let mut port = setup_nano();

    ////////// data setup //////////

    // testing data for easier debugging
    // let data = vec![
    //     0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
    //     0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
    //     0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
    //     0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
    // ];

    // from file -> Transmission
    let data = read_stdin_as_vec_u8().unwrap();

    let chunked = chunk_data(data, CHUNK_SIZE);

    let transmission = Transmission::new(make_transmission(chunked), false);
    let mut transmission_bins = transmission.clone().to_binary();

    transmission_bins = ready_for_send(transmission_bins);

    for _ in 0..100 {
        // TODO: iwann entfernen oder weniger
        transmission_bins.insert(0, 0);
        transmission_bins.insert(0, 0b1000);
    }
    let pb = ProgressBar::new((transmission_bins.len() - 1) as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{wide_bar}] [{percent}%] [{elapsed}|{eta}] [{bytes_per_sec}] [{pos}/{len}]")
            .unwrap()
            .progress_chars("=>-"),
    );

    ////////// timeout //////////
    let mut start_timeout = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    ////////// main loop //////////
    let mut received: Vec<u8> = Vec::new();

    read_stdin_as_vec_u8().expect("dumm"); // TODO: zum Testen

    let mut previous_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    loop {
        ////////// send //////////
        let current_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        // Check if the interval has elapsed
        if current_millis - previous_millis >= CLK_DELAY {
            previous_millis = current_millis;
            if !transmission_bins.is_empty() {
                let byte = transmission_bins.remove(0);

                start_timeout = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_millis();

                pb.suspend(|| {
                    // send_b15(&mut drv, byte);
                    send_nano(&mut port, byte);
                });
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
                start_timeout = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_millis();
                // pb.suspend(|| {
                //     eprint!("Received:{byte:2?} - [");
                //     print_colored_byte(byte);
                // });
                received.push(byte);
                if received.len() < 6 {
                    continue;
                }
                let mut range = None;
                pb.suspend(|| {
                    range = start_and_end(&received);
                });
                if let Some((start, end)) = range {
                    let data = received.clone();
                    received.clear();
                    // INFO: this returns ids of packets that are not recoverable or missing
                    pb.suspend(|| {
                        broken_ids = auswertung(
                            data,
                            start,
                            end,
                            &mut transmission_packet_array,
                            &transmission,
                            &mut transmission_bins,
                            &mut state,
                        );
                    });
                    pb.set_position(0);
                    pb.set_length(transmission_bins.len() as u64);

                    if broken_ids.is_empty() {
                        pb.finish();
                    } else {
                        pb.suspend(|| {
                            info!("Need {} packets to be resent!", broken_ids.len());
                        });
                        let chunked = chunk_data(u16_to_u8_vec(broken_ids.clone()), CHUNK_SIZE);
                        let transmission = Transmission::new(make_transmission(chunked), true);
                        transmission_bins.extend(ready_for_send(transmission.clone().to_binary()));
                        state = State::WaitingForResponse;
                        pb.set_position(0);
                        pb.set_length(transmission_bins.len() as u64);
                    }
                }
                // pb.suspend(|| {
                //     eprintln!("]");
                // });
            }
            Err(_e) => (),
        }

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        if current_time - start_timeout >= TIMEOUT {
            start_timeout = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();
            if state == State::WaitingForResponse {
                pb.suspend(|| {
                    info!("Timeout: resending Enquiry!");
                });
                received.clear();
                let chunked = chunk_data(u16_to_u8_vec(broken_ids.clone()), CHUNK_SIZE);
                let transmission = Transmission::new(make_transmission(chunked), true);
                transmission_bins.extend(ready_for_send(transmission.clone().to_binary()));
                pb.set_position(0);
                pb.set_length(transmission_bins.len() as u64);
            }
        }
    }
}

////////// nano functions //////////
#[allow(dead_code)]
fn setup_nano() -> Box<dyn SerialPort> {
    let mut port = serialport::new(PORT_NAME, BAUD_RATE)
        .timeout(Duration::from_millis(
            (CLK_DELAY - 3 * CLK_DELAY / 4) as u64,
        ))
        .open()
        .unwrap();

    port.clear(ClearBuffer::Input)
        .expect("port input buffer clear panicked");
    port.clear(ClearBuffer::Output)
        .expect("port output buffer clear panicked");

    port.write_all(&[0xFF]).expect("port write panicked");

    eprintln!("Serial port opened at {}", Yellow.paint(PORT_NAME));
    port
}

#[allow(dead_code)]
fn send_nano(port: &mut Box<dyn SerialPort>, byte: u8) {
    // eprint!("Send: [");
    // print_colored_byte(byte & 0xF);
    // eprintln!("]");
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
    // eprint!("Send: [");
    // print_colored_byte(byte & 0xF);
    // eprintln!("]");
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
#[allow(clippy::too_many_lines)]
fn auswertung(
    data: Vec<u8>,
    start: usize,
    end: usize,
    transmission_packet_array: &mut Vec<Vec<u8>>,
    init_transmission: &Transmission,
    transmission_bins: &mut Vec<u8>,
    state: &mut State,
) -> Vec<u16> {
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
            if msg.len() > 128 {
                error!("Packet too long: {}", msg.len());
                continue;
            }
            let decoded = decoder.correct_err_count(&msg, None);
            match decoded {
                Ok(content) => {
                    let buffer = content.0;
                    let errors = content.1;
                    if errors > 0 {
                        info!(
                            "Repaired Packet {}, had {} errors!",
                            packet.header.id, errors
                        );
                    }
                    buffer.data().to_vec()[header_vec.len()..].clone_into(&mut packet.data);
                    packet.ecc = buffer.ecc().to_vec();
                    let local_ids: Vec<u16> = packet
                        .data
                        .chunks(2)
                        .map(|x| u16::from_le_bytes([x[0], x[1]]))
                        .collect();
                    ids.extend(local_ids);
                }
                Err(e) => {
                    let id = packet.header.id;
                    info!("Packet {id} unrecoverable: {e:?}\n{packet:?}");
                }
            }
        }
        // respond with data for requested packets
        let mut transmission_now = init_transmission.clone();
        for i in (1..=transmission_now.packets.len()).rev() {
            assert!(i < u16::MAX as usize, "ID too large! (What did you do?)");
            if !ids.contains(&(i as u16)) {
                transmission_now.packets.retain(|x| x.header.id != i as u16);
            }
        }
        transmission_bins.extend(ready_for_send(transmission_now.clone().to_binary()));
        info!("Responding to Enquiry...");
    } else {
        *state = State::Normal;
        for mut packet in transmission.packets {
            let decoder = Decoder::new(packet.header.ecc_size as usize);
            let header_vec = packet.header.to_vec();
            let mut msg = header_vec.clone();
            msg.append(&mut packet.data.clone());
            msg.append(&mut packet.ecc.clone());
            if msg.len() > 128 {
                error!("Packet too long: {}", msg.len());
                continue;
            }
            let decoded = decoder.correct_err_count(&msg, None);
            match decoded {
                Ok((buffer, errors)) => {
                    if errors > 0 {
                        info!("Packet {} had {} errors!", packet.header.id, errors);
                    }
                    buffer.data().to_vec()[header_vec.len()..].clone_into(&mut packet.data);
                    packet.ecc = buffer.ecc().to_vec();
                    info!(
                        "{} ({}/{})",
                        Yellow.paint("Packet OK"),
                        packet.header.id,
                        transmission.header.total_packets
                    );
                    transmission_packet_array[packet.header.id as usize] = packet.data;
                }
                Err(e) => {
                    let id = packet.header.id;
                    info!("Packet {id} unrecoverable: {e:?}\n{packet:?}");
                }
            }
        }
    }
    let mut unrepairable_packets: Vec<u16> = Vec::new();
    let total_packets = transmission.header.total_packets;
    for packet_id in 1..=total_packets {
        // 1 - da es kein packet mit id 0 gibt
        if transmission_packet_array[packet_id as usize] == Vec::new() {
            unrepairable_packets.push(packet_id);
        }
    }

    if unrepairable_packets.is_empty() && !transmission.header.is_enquiry {
        let result: Vec<u8> = transmission_packet_array.concat();
        let mut stdout = io::stdout();
        info!("Writing data to stdout...");
        stdout.write_all(&result).expect("write failed");
        stdout.flush().expect("flush failed");
    }

    unrepairable_packets
}
