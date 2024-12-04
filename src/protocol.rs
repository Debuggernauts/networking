use crate::{
    controls, fatal, nibble,
    utilities::{nibbles_to_bytes, split_u16},
};
use ansi_term::Color::Green;
use ansi_term::Colour::Red;
use reed_solomon::Encoder;

#[derive(Debug, Clone)]
pub struct TransmissionHeader {
    pub is_enquiry: bool,
    pub total_packets: u16,
    pub ecc: Vec<u8>, // 4 bytes to safe 2 bytes
}

#[derive(Debug, Clone)]
pub struct Transmission {
    pub header: TransmissionHeader,
    pub packets: Vec<Packet>,
}

#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub size: u16,
    pub id: u16,
    pub ecc_size: u8,
}

#[derive(Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
    pub data: Vec<u8>,
    pub ecc: Vec<u8>,
}

impl PacketHeader {
    pub fn new(size: u16, id: u16, ecc_size: u8) -> Self {
        Self { size, id, ecc_size }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        vec![
            controls::SOH,
            (self.size >> 8) as u8,
            self.size as u8,
            (self.id >> 8) as u8,
            self.id as u8,
            self.ecc_size,
            controls::SOTX,
        ]
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn to_binary(&self) -> Vec<(u8, bool)> {
        vec![
            (controls::SOH, true),
            ((self.size >> 8) as u8, false),
            (self.size as u8, false),
            ((self.id >> 8) as u8, false),
            (self.id as u8, false),
            (self.ecc_size, false),
            (controls::SOTX, true),
        ]
    }

    pub fn empty() -> Self {
        Self {
            size: 0,
            id: 0,
            ecc_size: 0,
        }
    }
}

impl Packet {
    pub fn new(packet_data: Vec<u8>, id: u16) -> Self {
        let data_size = packet_data.len();
        let ecc_size = (data_size as f32 * 1.5) as usize;
        // data size encoded is 3 times the size of the data
        let header = PacketHeader::new(((data_size as f32) * 1.5_f32) as u16, id, ecc_size as u8);

        let mut complete_data = Vec::new();
        complete_data.append(&mut header.to_vec());
        complete_data.append(&mut packet_data.clone());

        let encoded = Encoder::new(ecc_size.clone()).encode(&complete_data[..]);

        Self {
            header: PacketHeader::new((data_size * 3) as u16, id, ecc_size as u8),
            data: packet_data,
            ecc: encoded.ecc().to_vec(),
        }
    }

    pub fn from_binary(data: Vec<Vec<u8>>) -> Vec<Self> {
        let mut packets: Vec<Packet> = Vec::new();

        for i in (0..data.len()-1).step_by(2) {
            let header = data[i + 0].clone();
            let size: u16 = ((header[1] as u16) << 8 | (header[2] as u16))/3*2;
            let id: u16 = (header[3] as u16) << 8 | (header[4] as u16);
            let ecc_size: u8 = header[5];
            let bytes = data[i + 1].clone();
            let pack_header = PacketHeader {
                size,
                id,
                ecc_size,
            };
            let data = bytes[1..(size/3 + 1) as usize].to_vec();
            let ecc = bytes[(size/3 + 1) as usize..].to_vec();

            let packet = Packet {
                header: pack_header,
                data,
                ecc,
            };
            dbg!(&packet);
            packets.push(packet);
        }
        /*println!("data (from_binary): ");
        for elem in &packets {
            println!("  -> {:?}", elem);
        }*/
        packets
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
            ecc: encoded.ecc().to_vec(),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn to_binary(&self) -> Vec<(u8, bool)> {
        let mut binary = vec![
            (controls::SOH, true),
            (self.is_enquiry as u8, false),
            ((self.total_packets >> 8) as u8, false),
            (self.total_packets as u8, false),
        ];
        binary.append(&mut self.ecc.iter().map(|byte| (*byte, false)).collect());
        binary
    }
}

impl Transmission {
    pub fn new(data: Vec<Packet>, is_enquiry: bool) -> Self {
        Self {
            header: TransmissionHeader::new(data.len() as u16, is_enquiry),
            packets: data,
        }
    }

    pub fn from_bytes(data: Vec<u8>) { //  , byte_map: HashMap<u8, &str>
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

        binary.extend(self.header.to_binary());
        binary.extend(
            self.packets
                .iter()
                .flat_map(|packet| packet.to_binary())
                .collect::<Vec<(u8, bool)>>(),
        );
        binary.push((controls::EOT, true));

        dbg!(&binary);

        let mut clock: u8 = 0b0;
        let mut buffer: Vec<u8> = Vec::new();

        for mapped_byte in binary {
            let byte = mapped_byte.0;
            let is_control = mapped_byte.1;
            let one: u8 = (clock << 3) | (nibble!(byte >> 4).1 >> 1);
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
    /// data: raw data (ohne nullen aka full bytes )
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
/*
protokoll_bytes: [3, 7, 9, 14, 1, 0, 0, 5, 51, 238, 133, 93, 1, 0, 30, 0, 1, 15, 2, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 252, 198, 178, 116, 134, 120, 188, 136, 90, 191, 67, 51, 75, 88, 168, 1, 0, 30, 0, 2, 15, 2, 100, 1, 2, 3, 4, 5, 6, 7, 8, 9, 112, 14, 78, 11, 194, 239, 76, 149, 16, 34, 106, 31, 70, 50, 131, 1, 0, 30, 0, 3, 15, 2, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 186, 109, 94, 99, 180, 168, 9, 123, 38, 204, 175, 19, 110, 103, 194, 1, 0, 30, 0, 4, 15, 2, 9, 10, 0, 1, 2, 3, 4, 5, 6, 7, 73, 14, 74, 126, 41, 55, 190, 166, 206, 242, 91, 150, 75, 39, 247, 1, 0, 9, 0, 5, 4, 2, 8, 9, 10, 179, 191, 168, 171, 4, ]
fucking 3
 */
        print!("protokoll_bytes: [");
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
            transmission: None,
        }
    }

    pub fn decode(&mut self) -> Transmission {
        let chunks = split_data(self.bytes.clone(), self.flags.clone());

        let trans_header_chunks = chunks[0..2].to_vec();
        let mut transmission_header: Option<TransmissionHeader> = None;
        for chunk in trans_header_chunks {
            if chunk[0] == 1u8 {
                let is_enquiry: bool = chunk[1] & 0b1 == 1;
                //dbg!(&chunk);
                let total_packets: u16 = (chunk[2] as u16) << 8 | (chunk[3] as u16);
                let ecc: Vec<u8> = chunk[4..].to_vec();
                transmission_header = Some(TransmissionHeader {
                    is_enquiry,
                    total_packets,
                    ecc,
                });
                break;
            }
        }
        if transmission_header.is_none() {
            fatal!("Transmission header not found");
        }
        let packets: Vec<Packet> = Packet::from_binary(chunks[2..chunks.len() - 1].to_vec());

        let transmission: Transmission = Transmission {
            header: transmission_header.unwrap(),
            packets
        };
        transmission
    }
}

/// Splits data at each control sequence
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
