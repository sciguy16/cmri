Prerequisites:

```
sudo apt install gcc-avr avr-libc
```

Build:

```
rustup override set nightly
rustup component add rust-src

# Compile the crate to an ELF executable.
cargo +nightly build -Z build-std=core --target avr-atmega328p.json --release
```

Flash:
```
avrdude -patmega328p -cusbasp -b115200 -D -Uflash:w:target/avr-atmega328p/release/blink.elf:e
```

