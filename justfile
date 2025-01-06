set windows-shell := ["pwsh.exe","-c"]
check:
    cargo clippy --all -- -W clippy::all -W clippy::pedantic
run:
    cargo run
    sudo chown $USER /dev/ttyUSB0
