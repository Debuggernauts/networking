use std::{
    io,
    io::{
        BufRead,
        Read
    },
    collections::HashMap
};
use crate::{
    nibble,
    protocol::Packet,
};

/// input 3 raw bytes, get 2 decoded bytes
/// approved: da liegt nicht der fehler ihr Deppen ;)
pub fn nibbles_to_bytes(nibbles: [u8; 3]) -> Vec<(u8, bool)> {
    let mut first_byte = (nibble!(nibbles[0]).0 & 0b0111) << 5;
    first_byte |= (nibble!(nibbles[0]).1 & 0b0111) << 2;
    first_byte |= (nibble!(nibbles[1]).0 & 0b0110) >> 1;
    let mut second_byte = (nibble!(nibbles[1]).1 & 0b0111) << 5;
    second_byte |= (nibble!(nibbles[2]).0 & 0b0111) << 2;
    second_byte |= (nibble!(nibbles[2]).1 & 0b0110) >> 1;

    let is_control_one: bool = nibble!(nibbles[1]).0 & 0b1 == 1;
    let is_control_two: bool = nibble!(nibbles[2]).1 & 0b1 == 1;
    vec![
        (first_byte, is_control_one),
        (second_byte, is_control_two),
    ]
}

pub fn read_bin_file(file_path: &str) -> Vec<u8> {
    let mut file = std::fs::File::open(file_path).expect("Couldn't open file!");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("Couldn't read binary file!"); 
    buffer
}

pub fn read_stdin_as_vec_u8() -> io::Result<Vec<u8>> {
    let mut buffer: Vec<u8> = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(buffer)
}

pub fn read_pipe() -> String {
    let stdin = io::stdin(); 
    let handle = stdin.lock();

    for line in handle.lines() {
        match line {
            Ok(line) => return line,
            Err(e) => {
                eprintln!("Error reading line: {}", e);
                break;
            }
        }
    }
    "".to_string()
}

/*pub fn make_transmission(data: Vec<Vec<u8>>) -> Vec<Packet> {
    let mut id = 0;
    let mut transmission = Vec::new();
    let start = Packet::new(vec![controls::SOT], 0);
    
    transmission.push(start);
    


    let mut packets = Vec::new();
    for packet in data {
        id += 1;
        packets.push(Packet::new(packet, id))
    }
    transmission.extend(packets);
    id += 1;
    let end = Packet::new(vec![controls::EOT], id);
    transmission.push(end);
    transmission
}*/

pub fn make_transmission(data: Vec<Vec<u8>>) -> Vec<Packet> {
    let mut id = 0;
    let mut packets = Vec::new();
    for packet in data {
        id += 1;
        packets.push(Packet::new(packet,id))
    }
    packets
}

pub fn split_u16(bytes: u16) -> [u8; 2] {
    let high_byte = (bytes >> 8) as u8;
    let low_byte = (bytes & 0xFF) as u8;
    [high_byte, low_byte]
}

pub fn chunk_data(data: Vec<u8>, size: usize) -> Vec<Vec<u8>> {
    let chunks: Vec<Vec<u8>> = data.chunks(size)
        .map(|chunk| chunk.to_vec())  // Convert each chunk into a Vec<u8>
        .collect();

    chunks
}

pub fn slice_vec(input: Vec<u8>, sizes: Vec<usize>) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut start = 0;

    for &size in sizes.iter() {
        let end = start + size;
        if end > input.len() {
            break;
        }
        result.push(input[start..end].to_vec());
        start = end;
    }

    result
}

pub fn bytes_to_binary_str(data: Vec<u8>) -> String {
    data.iter()
        .map(|byte| format!("{:08b}", byte))
        .collect::<Vec<String>>()
        .join("")
}

pub fn binary_str_to_bytes(data: String) -> Vec<u8> {
    data.as_bytes()
        .chunks(8)
        .map(|chunk| {
            let byte_str = std::str::from_utf8(chunk).unwrap();
            u8::from_str_radix(byte_str, 2)
        })
        .collect::<Result<Vec<u8>,_>>()
        .unwrap()
}

pub fn debug_print(transmission: Vec<u8>, control: HashMap<u8, &str>) {
    let binary_str: String = transmission.iter()
        .map(|byte| format!("{:08b}", byte))  // Convert each byte to a binary string of 8 bits
        .collect();

    let nibbles: Vec<String> = binary_str.chars()
        .collect::<Vec<char>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect();

    let mut grouped: Vec<Vec<String>> = nibbles.chunks(3)
        .map(|chunk| chunk.to_vec())  // Convert each chunk of 3 nibbles into a Vec<String>
        .collect();

    for group in &mut grouped {
        if group.len() < 2 {
            continue
        }
        let combined = group.clone().join("");
        let data = &get_data(combined.clone());
        group[1] += &format!("\t{}", data);
        if (group.len() == 3) && (group[2].chars().nth(3).unwrap() == '1') {
            group[1] += &format!("\t{}", control.get(&u8::from_str_radix(get_data(combined).as_str(), 2).unwrap()).unwrap());
        }
        else if group.len() == 3 {
            group[1] += &format!("\t{}", u8::from_str_radix(get_data(combined).as_str(), 2).unwrap())
        }
    }

    for group in &grouped {
        for i in 0..group.len() {
            println!("{}", group[i]);
        }
        println!("---");
    }

    
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
