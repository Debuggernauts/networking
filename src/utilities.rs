use std::{io, io::Read};

use ansi_term::Color::{Blue, Green, Red, Yellow};

use crate::{nibble, protocol::Packet};

/// input 3 raw bytes, get 2 decoded bytes
pub fn nibbles_to_bytes(nibbles: [u8; 3]) -> Vec<(u8, bool)> {
    let mut first_byte = (nibble!(nibbles[0]).0 & 0b0111) << 5;
    first_byte |= (nibble!(nibbles[0]).1 & 0b0111) << 2;
    first_byte |= (nibble!(nibbles[1]).0 & 0b0110) >> 1;
    let mut second_byte = (nibble!(nibbles[1]).1 & 0b0111) << 5;
    second_byte |= (nibble!(nibbles[2]).0 & 0b0111) << 2;
    second_byte |= (nibble!(nibbles[2]).1 & 0b0110) >> 1;

    let is_control_one: bool = nibble!(nibbles[1]).0 & 0b1 == 1;
    let is_control_two: bool = nibble!(nibbles[2]).1 & 0b1 == 1;
    vec![(first_byte, is_control_one), (second_byte, is_control_two)]
}

pub fn read_stdin_as_vec_u8() -> io::Result<Vec<u8>> {
    let mut buffer: Vec<u8> = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(buffer)
}

pub fn make_transmission(data: Vec<Vec<u8>>) -> Vec<Packet> {
    let mut id = 0;
    let mut packets = Vec::new();
    for packet in data {
        id += 1;
        packets.push(Packet::new(packet, id));
    }
    packets
}

pub fn split_u16(bytes: u16) -> [u8; 2] {
    let high_byte = (bytes >> 8) as u8;
    let low_byte = (bytes & 0xFF) as u8;
    [high_byte, low_byte]
}

pub fn chunk_data(data: Vec<u8>, size: usize) -> Vec<Vec<u8>> {
    let chunks: Vec<Vec<u8>> = data
        .chunks(size)
        .map(<[u8]>::to_vec) // Convert each chunk into a Vec<u8>
        .collect();

    chunks
}

fn get_data(encoded_string: String) -> String {
    let mut data = String::new();
    data += &String::from(encoded_string.chars().nth(1).unwrap());
    data += &String::from(encoded_string.chars().nth(2).unwrap());
    data += &String::from(encoded_string.chars().nth(3).unwrap());
    data += &String::from(encoded_string.chars().nth(5).unwrap());
    data += &String::from(encoded_string.chars().nth(6).unwrap());
    data += &String::from(encoded_string.chars().nth(7).unwrap());
    data += &String::from(encoded_string.chars().nth(9).unwrap());
    data += &String::from(encoded_string.chars().nth(10).unwrap());
    data
}

pub fn slice_data(received: Vec<u8>, start: usize, end: usize) -> Vec<u8> {
    received
        .get(start..end + 3)
        .expect("Failed to slice received data")
        .to_vec()
}

/// checks if the data has a start and end
pub fn start_and_end(p0: &Vec<u8>) -> Option<(usize, usize)> {
    let mut start_found = false;
    let mut start_index = 0;
    let mut end_found = false;
    let mut end_index = 0;
    for i in 0..p0.len() - 2 {
        let nibble_0 = p0[i];
        let nibble_1 = p0[i + 1];
        let nibble_2 = p0[i + 2];
        // there are 2 theoretically possible combinations of SOT (main difference being clocked)
        if (nibble_0 == 0b0 || nibble_0 == 0b1000)
            && (nibble_1 == 0b0 || nibble_1 == 0b1001)
            && (nibble_2 == 0b111 || nibble_2 == 0b1111)
            && !start_found
        {
            // found SOT
            start_found = true;
            //eprint!("{} {}", Yellow.paint("start_found".to_string()), i);
            start_index = i;
            if start_found && (p0.len() - start_index) % 3 == 0 {
                // eprintln!("\n---------------------");
            }
            break;
        }
    }

    for i in (start_index..p0.len() - 2).rev() {
        let nibble_0 = p0[i];
        let nibble_1 = p0[i + 1];
        let nibble_2 = p0[i + 2];

        if (nibble_0 == 0b0 || nibble_0 == 0b1000)
            && (nibble_1 == 0b1001 || nibble_1 == 0b1)
            && (nibble_2 == 0b1 || nibble_2 == 0b1001)
        {
            // found EOT
            if start_found && (i - start_index) % 3 == 0 {
                end_found = true;
                //eprintln!("{} {}", Blue.paint("end_found".to_string()), i);
                end_index = i;

                break;
            }
        }
    }

    if start_found && end_found {
        Some((start_index, end_index))
    } else {
        None
    }
}

pub fn print_colored_byte(byte: u8) {
    let bits: Vec<String> = (0..4)
        .rev()
        .map(|i| {
            let bit = (byte >> i) & 1;
            if i == 3 {
                if bit == 1 {
                    Green.paint(format!("{bit}")).to_string()
                } else {
                    Red.paint(format!("{bit}")).to_string()
                }
            } else {
                format!("{}", bit)
            }
        })
        .collect();

    // Join the colored bits into a string
    eprint!("{}", bits.join(""));
}

pub fn u16_to_u8_vec(input: Vec<u16>) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len() * 2);
    for value in input {
        output.push((value & 0xFF) as u8); // Niedriges Byte
        output.push((value >> 8) as u8); // Hohes Byte
    }
    output
}

pub fn ready_for_send(transmission_bins: Vec<u8>) -> Vec<u8> {
    let new_transmission_bins: Vec<u8> = transmission_bins
        .iter()
        .flat_map(|&byte| {
            // Oberes und unteres Nibble berechnen
            // eprintln!("byte: {:08b}", byte);
            let upper_nibble = byte >> 4;
            let upper_clock = upper_nibble & 0b1000;
            let lower_nibble = byte & 0xF;
            let lower_clock = lower_nibble & 0b1000;

            // eprintln!("upper: {:04b}", upper_nibble | (!upper_clock & 0b1000));
            // eprintln!("upclk: {:04b}", upper_nibble);
            // eprintln!("lower: {:04b}", lower_nibble & ((!lower_clock & 0b1000) | 0b0111));
            // eprintln!("lwclk: {:04b}", lower_nibble);

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
