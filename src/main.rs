use std::io::{BufRead,Read};
use std::io;
use reed_solomon::{Encoder, Decoder};
use b15r::{
    B15F,
    DdrPin::DDRA,
    PinPin::PINA,
    Port0
};
use b15r::PortPin::PORTA;
use v7::{
    info,
    error,
    fatal,
    nibble
};
use std::thread::sleep;
use std::time::Duration;
use v7::controls;
use indicatif::{ProgressBar, ProgressStyle};

const MAX_SIZE: u16 = 128;  // packet size in bytes;
const PACKET_HEADER_SIZE: usize = 4; // in bytes
const BYTE_EXPANSION: usize = 4; // in bits

#[derive(Debug, Clone)]
struct TransmissionHeader {
    total_packets: u16,
}

#[derive(Debug, Clone)]
struct Transmission {
    header: TransmissionHeader,
    packets: Vec<Packet>,
}

#[derive(Debug, Clone)]
struct PacketHeader {
    size: u16,
    id: u16,
    ecc_size: u8,
}

#[derive(Debug, Clone)]
struct Packet {
    header: PacketHeader,
    data: Vec<u8>,
    ecc: Vec<u8>,
}

impl PacketHeader {
    pub fn new(size: u16, id: u16, ecc_size: u8) -> Self {
        Self { size, id, ecc_size }
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
}

impl Packet {
    pub fn new(packet_data: Vec<u8>, id: u16) -> Self {
        let data_size = PACKET_HEADER_SIZE + packet_data.len();
        let ecc_size = (data_size as f32 * 0.5) as usize;
        let encoded = Encoder::new(ecc_size.clone()).encode(&packet_data[..]);
        Self {
            header: PacketHeader::new(data_size as u16, id, ecc_size as u8),
            data: encoded.data().to_vec(),
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
        binary.push((controls::EOTX, true));
        binary
    }

    pub fn set_size(&mut self, new_size: u16) {
        self.header.size = new_size;
    }
}

impl TransmissionHeader {
    pub fn new(size: usize) -> Self {
        Self {
            total_packets: size as u16
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn to_binary(&self) -> Vec<(u8, bool)> {
        vec![
            (controls::SOT, true),
            (controls::SOH, true),
            (((self.total_packets >> 8) as u8), false),
            ((self.total_packets as u8), false),
        ]
    }
}

impl Transmission {
    pub fn new(data: Vec<Packet>) -> Self {
        Self {
            header: TransmissionHeader::new(data.len()),
            packets: data
        }
    }

    /*fn create_packet(&self) -> Packet {

    }*/

    fn set_packets(&mut self, packets: Vec<Packet>) {
        self.packets = packets;
    }

    /// C = clock, I = is_control, D = data
    /// CDDD
    /// CDDD
    /// CDDI
    pub fn to_binary(&self) -> Vec<u8> {
        let mut binary: Vec<(u8, bool)> = Vec::new();
        binary.extend(self.header.to_binary());
        let temp_packs = self.packets.clone();
        for mut packet in temp_packs {
            packet.set_size(((packet.header.size as f64) * 1.5) as u16);
        }
        binary.extend(
            self.packets
                .iter()
                .map(|packet| packet.to_binary())
                .flatten()
                .collect::<Vec<(u8, bool)>>(),
        );
        binary.push((controls::EOT, true));

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

fn read_stdin_as_vec_u8() -> io::Result<Vec<u8>> {
    let mut buffer: Vec<u8> = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(buffer)
}



fn read_pipe() -> String {
    let stdin = std::io::stdin(); 
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
    return "".to_string();
}

fn read_bin_file(file_path: &str) -> Vec<u8> {
    let mut file = std::fs::File::open(file_path).expect("Couldn't open file!");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("Couldn't read binary file!"); 
    buffer
}

fn make_transmission(data: Vec<Vec<u8>>) -> Vec<Packet> {
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
}

fn split_u16(bytes: u16) -> (u8, u8) {
    let high_byte = (bytes >> 8) as u8;
    let low_byte = (bytes & 0xFF) as u8;
    (high_byte, low_byte)
}

fn chunk_data(data: Vec<u8>) -> Vec<Vec<u8>> {
    let mut chunks: Vec<Vec<u8>> = data.chunks(60)
        .map(|chunk| chunk.to_vec())  // Convert each chunk into a Vec<u8>
        .collect();

    chunks
}

fn bytes_to_binary_str(data: Vec<u8>) -> String {
    data.iter()
        .map(|byte| format!("{:08b}", byte))
        .collect::<Vec<String>>()
        .join("")
}

fn binary_str_to_bytes(data: String) -> Vec<u8> {
    data.as_bytes()
        .chunks(8)
        .map(|chunk| {
            let byte_str = std::str::from_utf8(chunk).unwrap();
            u8::from_str_radix(byte_str, 2)
        })
        .collect::<Result<Vec<u8>,_>>()
        .unwrap()
}

fn main() {
    let mut drv = B15F::get_instance();
    drv.set_register(DDRA, 0x0F);

    let message = "wagihwp".to_string().into_bytes();
    let chunked = chunk_data(message);

    let data = read_stdin_as_vec_u8().unwrap();
    /*println!("{:?}", data.len());
    println!("{:?}", Transmission::new(make_transmission(chunk_data(data.clone()))).to_binary());
    println!("{:?}", Transmission::new(make_transmission(chunk_data(data))).to_binary().len());
    */
    let transmission_bins = Transmission::new(make_transmission(chunk_data(data))).to_binary();
    /*let pb = ProgressBar::new(transmission_bins.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{wide_bar} {percent}%").unwrap()
        .progress_chars("=>-"));
*/
    for byte in &transmission_bins {
        //print!("{:08b}, ", byte);
        drv.set_register(PORTA, (byte & 0xF0) >> 4);
        sleep(Duration::from_millis(50));
        drv.set_register(PORTA, byte & 0x0F);
        sleep(Duration::from_millis(50));
     //   pb.inc(1);
    }
    //pb.finish_with_message("Done!");

    

    /*println!("{:?}", chunked);
    dbg!(Transmission::new(make_transmission(chunked.clone())));
    println!("{:?}", Transmission::new(make_transmission(chunked)).to_binary());
    */
    //let enc = Encoder::new(8);
    //let encoded = enc.encode(&message[..]);


    /*let enc = Encoder::new(8);
    let dec = Decoder::new(8);

    let pipe = read_pipe().into_bytes();
    let encoded = enc.encode(&pipe[..]);

    println!("{:?}", encoded.data());
*/
    /*for e in encoded.data() {
        sleep(Duration::from_millis(1000));
        info!("{:08b}", e);
        drv.set_register(PORTA, *e as u8);
        sleep(Duration::from_millis(1000));
        drv.set_register(PORTA, (*e >> 4) as u8);
    }*/
}
