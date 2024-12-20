use std::collections::HashMap;
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

#[derive(Debug, Clone)]
pub struct TransmissionHeader {
    total_packets: u16,
    ecc: Vec<u8> // 4 bytes to safe 2 bytes
}

#[derive(Debug, Clone)]
pub struct Transmission {
    header: TransmissionHeader,
    packets: Vec<Packet>,
}

#[derive(Debug, Clone)]
pub struct PacketHeader {
    size: u16,
    id: u16,
    ecc_size: u8,
}

#[derive(Debug, Clone)]
pub struct Packet {
    header: PacketHeader,
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

    pub fn from_binary(data: String) -> Self {
        todo!()
    }

    pub fn to_binary(&self) -> Vec<(u8, bool)> {
        let mut binary: Vec<(u8, bool)> = Vec::new();
        binary.append(&mut self.header.to_binary());
        binary.append(&mut self.data.iter().map(|byte| (*byte, false)).collect());
        binary.append(&mut self.ecc.iter().map(|byte| (*byte, false)).collect()); // TODO CANT EXTEND
        binary
    }

    pub fn set_size(&mut self, new_size: u16) {
        self.header.size = new_size;
    }
}

impl TransmissionHeader {
    pub fn new(size: u16) -> Self {
        let encoded = Encoder::new(4).encode(&split_u16(size.clone())[..]);
        Self {
            total_packets: size,
            ecc: encoded.ecc().to_vec()
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn to_binary(&self) -> Vec<(u8, bool)> {
        let mut binary = vec![
            (controls::SOH, true),
            (((self.total_packets >> 8) as u8), false),
            ((self.total_packets as u8), false),
        ];
        binary.append(&mut self.ecc.iter().map(|byte| (*byte, false)).collect());
        binary
    }
}

impl Transmission {
    pub fn new(data: Vec<Packet>) -> Self {
        Self {
            header: TransmissionHeader::new(data.len() as u16),
            packets: data
        }
    }

    pub fn from_bytes(data: Vec<u8>, byte_map: HashMap<u8, &str>) {
        let decoder = ProtocolDecoder::new(data, byte_map);


    }

    fn set_packets(&mut self, packets: Vec<Packet>) {
        self.packets = packets;
    }

    fn create_start() -> Vec<(u8, bool)> {
        let encoded = Encoder::new(2).encode(&[controls::SOT]);
        let mut binary: Vec<(u8, bool)> = Vec::new();
        binary.push((controls::SOT, true));
        binary.append(&mut encoded.ecc().iter().map(|byte| (*byte, false)).collect()); // TODO CANT EXTEND
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
                .map(|packet| packet.to_binary())
                .flatten()
                .collect::<Vec<(u8, bool)>>()
        );
        binary.push((controls::EOT, true));

        println!("{:?}", binary);

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
    pub fn new(data: Vec<u8>, byte_map: HashMap<u8, &str>) -> Self {
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
            flags.push(tuple.1);
        }

        Self {
            bytes,
            flags,
            transmission: None
        }
    }

    pub fn decode(&mut self) {
        let transmission_header: TransmissionHeader;
        
        let sliced = slice_vec(self.bytes.clone(), vec![SOT_SIZE,TRANSMISSION_HEADER_SIZE, self.bytes.len()-10]);
        let packets = &sliced[sliced.len()-1];
        let data_size = ((packets.len() as f32 - 15.0) / 2.5) as u16; // This formula is given by: packets.len() = PACKET_HEADER_SIZE + DATA_SIZE + 1.5 * (PACKET_HEADER_SIZE + DATA_SIZE)
        let ecc_size = (packets.len() - PACKET_HEADER_SIZE - data_size as usize) as u8;
        println!("{}", data_size);
        println!("{}", ecc_size);
    } 

}
