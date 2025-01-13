#[macro_export]
macro_rules! nibble {
    ($byte:expr) => {
        (($byte >> 4) & 0x0F, $byte & 0x0F)
    };
}

#[macro_export]
macro_rules! info {
    ($fmt:expr $(, $args:expr)*) => {
        eprintln!(
            "[{}] {}",
            "\x1b[32m\x1b[1mINFO\x1b[0m",
            format!($fmt, $($args),*)
        )
    };
}

#[macro_export]
macro_rules! error {
    ($fmt:expr $(, $args:expr)*) => {
        eprintln!(
            "[{}] {}",
            "\x1b[31m\x1b[1mERROR\x1b[0m",
            format!($fmt, $($args),*)
        )
    };
}

#[macro_export]
macro_rules! fatal {
    ($fmt:expr $(, $args:expr)*) => {
        eprintln!(
            "[{}] {}",
            "\x1b[31m\x1b[1m\x1b[5mFATAL\x1b[0m",
            format!($fmt, $($args),*)
        );
        std::process::exit(1);
    };
}
