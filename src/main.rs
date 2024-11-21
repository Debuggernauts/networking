use std::{
    io,
    io::{
        BufRead, 
        Read
    },
    thread::sleep,
    time::Duration,
    collections::HashMap
};
use reed_solomon::{
    Encoder,
    Decoder
};
use indicatif::{
    ProgressBar,
    ProgressStyle
};
use b15r::{
    B15F,
};
use v7::{
    controls,
    protocol::*,
    utilities::*,
};

fn main() {
    /*let mut drv = B15F::get_instance();
    drv.set_register(DDRA, 0x0F);*/

    //let message = "philippfabianundmaxverzweifelnimhwp".to_string().into_bytes();
    let message = "hwp".to_string().into_bytes();
    //println!("{}", message.clone().len());
    let chunks = chunk_data(message.clone(), 60);
    
    println!("{:08b}", message[0]);
    println!("{:08b}", message[1]);
    println!("{:08b}", message[2]);


    let data = read_stdin_as_vec_u8().unwrap();
    let mut transmission_bins = Transmission::new(make_transmission(chunk_data(message.clone(), 60)));
    println!("{:?}", make_transmission(chunk_data(message, 60)));
    

    /*let pb = ProgressBar::new(transmission_bins.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{wide_bar} {percent}%").unwrap()
        .progress_chars("=>-"));
    */
    let byte_map: HashMap<u8, &str> = HashMap::from([
        (controls::SOT, "SOT"),
        (controls::EOT, "EOT"),
        (controls::SOH, "SOH"),
        (controls::SOTX, "SOTX"),
        (controls::EOTX, "EOTX"),
        (controls::ENQ, "ENQ"),
        (controls::ACK, "ACK"),
        (controls::NAC, "NAC")
    ]);

    Transmission::from_bytes(transmission_bins.clone().to_binary(), byte_map.clone());
    println!("{:?}", transmission_bins.clone().to_binary().len() * 2  / 3);
    debug_print(transmission_bins.clone().to_binary(), byte_map.clone());

    //println!("{}", transmission_bins.to_binary().len());

    let mut protocol_decoder = ProtocolDecoder::new(transmission_bins.to_binary(), byte_map);
    //protocol_decoder.decode();

    //let smaller_buffer = vec![transmission_bins[0], transmission_bins[1], transmission_bins[3]];
    /*for byte in &smaller_buffer {
        //print!("{:08b}, ", byte);
        drv.set_register(PORTA, (byte & 0xF0) >> 4);
        sleep(Duration::from_millis(500));
        drv.set_register(PORTA, byte & 0x0F);
        sleep(Duration::from_millis(500));
     //   pb.inc(1);
    }*/

    //pb.finish_with_message("Done!");
}
