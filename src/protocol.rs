use std::collections::HashMap;
use ansi_term::Color::Green;
use ansi_term::Colour;
use reed_solomon::{
    Encoder,
    Decoder
};
use crate::{
    nibble,
    fatal,
    controls,
    consts::*,
    utilities::{
        nibbles_to_bytes,
        split_u16,
        slice_vec
    }
};
use ansi_term::Colour::Red;
use crate::utilities::make_transmission;

#[derive(Debug, Clone)]
pub struct TransmissionHeader {
    is_enquiry: bool,
    total_packets: u16,
    ecc: Vec<u8> // 4 bytes to safe 2 bytes
}

#[derive(Debug, Clone)]
pub struct Transmission {
    header: TransmissionHeader,
    pub packets: Vec<Packet>,
}

#[derive(Debug, Clone)]
pub struct PacketHeader {
    size: u16,
    pub id: u16,
    ecc_size: u8,
}

#[derive(Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
    data: Vec<u8>,
    ecc: Vec<u8>,
}

impl PacketHeader {
    pub fn new(size: u16, id: u16, ecc_size: u8) -> Self {
        Self { size, id, ecc_size }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        vec![
            controls::SOH,
            (self.size >> 8) as u8,
            (self.size as u8),
            (self.id >> 8) as u8,
            (self.id as u8),
            self.ecc_size,
            controls::SOTX
        ]
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn to_binary(&self) -> Vec<(u8, bool)> {
        vec![
            (controls::SOH, true),
            (((self.size >> 8) as u8), false),
            ((self.size as u8), false),
            (((self.id >> 8) as u8), false),
            ((self.id as u8), false),
            (self.ecc_size, false),
            (controls::SOTX, true),
        ]
    }

    pub fn empty() -> Self {
        Self {
            size: 0,
            id: 0,
            ecc_size: 0
        }
    }
}

impl Packet {
    pub fn new(packet_data: Vec<u8>, id: u16) -> Self {
        let data_size = packet_data.len();
        let ecc_size = (data_size as f32 * 1.5) as usize;
        let header = PacketHeader::new(data_size as u16 *2, id, ecc_size as u8);

        let mut complete_data = Vec::new();
        complete_data.append(&mut header.to_vec());
        complete_data.append(&mut packet_data.clone());

        let encoded = Encoder::new(ecc_size.clone()).encode(&complete_data[..]);

        Self {
            header: PacketHeader::new((data_size*3) as u16, id, ecc_size as u8),
            data: packet_data,
            ecc: encoded.ecc().to_vec(),
        }
    }

    pub fn from_binary(data: Vec<u8>) -> Vec<Self> {
        // SOH HEADER
        // size: u16,
        // pub id: u16,
        // ecc_size: u8,
        // SOTX
        // DATA
        let mut packets: Vec<Packet> = Vec::new();
        let mut i = 0;

        todo!()
    }

    pub fn to_binary(&self) -> Vec<(u8, bool)> {
        let mut binary: Vec<(u8, bool)> = Vec::new();
        binary.append(&mut self.header.to_binary());
        binary.append(&mut self.data.iter().map(|byte| (*byte, false)).collect());
        binary.append(&mut self.ecc.iter().map(|byte| (*byte, false)).collect());
        binary
    }

    pub fn set_size(&mut self, new_size: u16) {
        self.header.size = new_size;
    }
}

impl TransmissionHeader {
    pub fn new(size: u16, is_enquiry: bool) -> Self {
        let encoded = Encoder::new(4).encode(&split_u16(size.clone())[..]);
        Self {
            is_enquiry,
            total_packets: size,
            ecc: encoded.ecc().to_vec()
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn to_binary(&self) -> Vec<(u8, bool)> {
        let mut binary = vec![
            (controls::SOH, true),
            (self.is_enquiry as u8, false),
            (((self.total_packets >> 8) as u8), false),
            ((self.total_packets as u8), false),
        ];
        binary.append(&mut self.ecc.iter().map(|byte| (*byte, false)).collect());
        binary
    }
}

impl Transmission {
    pub fn new(data: Vec<Packet>, is_enquiry: bool) -> Self {
        Self {
            header: TransmissionHeader::new(data.len() as u16, is_enquiry),
            packets: data
        }
    }

    pub fn from_bytes(data: Vec<u8>, byte_map: HashMap<u8, &str>) {
        let mut decoder = ProtocolDecoder::new(data);
        decoder.decode();
    }

    fn set_packets(&mut self, packets: Vec<Packet>) {
        self.packets = packets;
    }

    fn create_start() -> Vec<(u8, bool)> {
        let encoded = Encoder::new(2).encode(&[controls::SOT]);
        let mut binary: Vec<(u8, bool)> = Vec::new();
        binary.push((controls::SOT, true));
        binary.append(&mut encoded.ecc().iter().map(|byte| (*byte, false)).collect());
        binary

    }

    /// C = clock, I = is_control, D = data
    /// CDDD
    /// CDDD
    /// CDDI
    pub fn to_binary(&self) -> Vec<u8> {
        let mut binary: Vec<(u8, bool)> = Vec::new();

        let start = Transmission::create_start();
        binary.extend(start);
        binary.extend(self.header.to_binary());
        binary.extend(
            self.packets
                .iter()
                .flat_map(|packet| packet.to_binary())
                .collect::<Vec<(u8, bool)>>()
        );
        binary.push((controls::EOT, true));
        
        dbg!(&binary);

        let mut clock: u8 = 0b0;
        let mut buffer: Vec<u8> = Vec::new();

        for mapped_byte in binary {
            let byte = mapped_byte.0;
            let is_control = mapped_byte.1;
            let one: u8 = (clock << 3) | (nibble!(byte >> 4).0 >> 1);
            clock ^= 1;
            let two: u8 = (clock << 3) | (nibble!(byte >> 2).1 & 0b0111);
            clock ^= 1;
            let three: u8 = (clock << 3) | (nibble!(byte << 1).1 & 0b110) | u8::from(is_control);
            clock ^= 1;
            buffer.push(one);
            buffer.push(two);
            buffer.push(three);
        }

        let mut result = Vec::new();
        for chunk in buffer.chunks(2) {
            if chunk.len() == 2 {
                // Schiebe das erste Nibble um 4 Bits nach links
                // und kombiniere es mit dem zweiten Nibble
                let combined = (chunk[0] << 4) | chunk[1];
                result.push(combined);
            } else {
                // Das letzte Nibble einfach nach links schieben
                let combined = chunk[0] << 4;
                result.push(combined);
            }
        }
        result
    }
}


pub struct ProtocolDecoder {
    bytes: Vec<u8>,
    flags: Vec<bool>,
    transmission: Option<Transmission>,
}

impl ProtocolDecoder {
    /// data: raw data (ohne nulln aka full bytes )
    pub fn new(data: Vec<u8>) -> Self {
        let byte_map = [
            controls::SOT,
            controls::EOT,
            controls::SOH,
            controls::SOTX,
            controls::EOTX,
            //controls::ENQ,
            //controls::ACK,
            //controls::NAC,
        ];

        let mut triplets = Vec::new();

        for chunk in data.chunks(3) {
            let mut arr = [0u8; 3];
            for (i, &item) in chunk.iter().enumerate() {
                arr[i] = item;
            }
            triplets.push(arr);
        }

        let mut tuple_vec = Vec::new();
        for triplet in triplets {
            let bytes = nibbles_to_bytes(triplet);
            tuple_vec.push(bytes[0]);
            tuple_vec.push(bytes[1]);
        }

        let mut bytes = Vec::new();
        let mut flags = Vec::new();

        for tuple in tuple_vec {
            bytes.push(tuple.0);
            if byte_map.contains(&tuple.0) {
                flags.push(tuple.1);
            } else {
                flags.push(false);
            }
        }

        if bytes.last() == Some(&0) {
            bytes.pop();
            flags.pop();
        }

        print!("prot_bytes: [");
        for i in 0..bytes.len() {
            let byte = bytes[i].to_string();
            if flags[i] {
                print!("{}, ", Red.paint(byte));
            } else {
                print!("{}, ", Green.paint(byte));
            }
        }
        println!("]");


        Self {
            bytes, // real, decoded data
            flags,
            transmission: None
        }
    }

    pub fn decode(&mut self) -> Transmission {
        /*
        [
        7, 9, 14,
        1, 0, 0, 5, 19, 14, 5, 29,
        1, 0, 30, 0, 1, 15,
        2, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 28, 6, 18, 20, 6, 24, 28, 8, 26, 31, 3, 19, 11, 24, 8,
        1, 0, 30, 0,
        2, 15, 2, 4, 1, 2, 3, 4, 5, 6, 7, 8, 9, 16, 14, 14, 11, 2, 15, 12, 21, 16, 2, 10, 31, 6, 18, 3,
        1, 0, 30, 0, 3, 15,
        2, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 26, 13, 30, 3, 20, 8, 9, 27, 6, 12, 15, 19, 14, 7, 2,
        1, 0, 30, 0, 4, 15,
        2, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7, 9, 14, 10, 30, 9, 23, 30, 6, 14, 18, 27, 22, 11, 7, 23,
        1, 0, 9, 0, 5, 4,
        2, 8, 9, 10, 19, 31, 8, 11,
        4,
        ]
         */
        let chunks = split_data(self.bytes, self.flags);
        for chunk in chunks {
        }

        let transmission_header: TransmissionHeader;
        let packets: Vec<Packet> = Vec::new();


        /// slices input stream into SOT, transmission header and packets
        let sliced = slice_vec(self.bytes.clone(), vec![SOT_SIZE,TRANSMISSION_HEADER_SIZE, self.bytes.len()-10]);
        /// u8 vec of raw packets
        let packets = &sliced[sliced.len()-1];
        let data_size = ((packets.len() as f32 - 15.0) / 2.5) as u16; // This formula is given by: packets.len() = PACKET_HEADER_SIZE + DATA_SIZE + 1.5 * (PACKET_HEADER_SIZE + DATA_SIZE)
        let ecc_size = (packets.len() - PACKET_HEADER_SIZE - data_size as usize) as u8;
        println!("data_size: {}", data_size);
        println!("ecc_size: {}", ecc_size);
        todo!();
        //let transmission: Transmission = Transmission::new(packets, );
        //transmission
    }

}

fn split_data<T: Clone>(data: Vec<T>, flags: Vec<bool>) -> Vec<Vec<T>> {
    if data.len() != flags.len() {
        panic!("Data and flags vectors must have the same length");
    }

    let mut chunks: Vec<Vec<T>> = Vec::new();
    let mut current_chunk: Vec<T> = Vec::new();

    for (value, flag) in data.into_iter().zip(flags.into_iter()) {
        if flag && !current_chunk.is_empty() {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
        }
        current_chunk.push(value);
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}
