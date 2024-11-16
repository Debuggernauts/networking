use std::io::BufRead;
use b15r::B15F;

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

fn b15f_init() -> (B15F, String) {
    (B15F::get_instance(), read_pipe())
}

fn main() {
    let (drv, pip) = b15f_init();
}
