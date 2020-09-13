use crate::{CmriStateMachine, MessageType, RxState};
use ruduino::legacy::serial;

/// Hardcode this for now. Only used to calculate baud rates for serial.
/// If other freqs are required then please open an issue
const CPU_FREQUENCY_HZ: u64 = 16_000_000;

/// Hardcode 64 in/64 out for now
const INPUT_BITS: u8 = 64;
const INPUT_BYTES: u8 = INPUT_BITS / 8;
const OUTPUT_BITS: u8 = 64;
const OUTPUT_BYTES: u8 = OUTPUT_BITS / 8;

/// Stores 64 input and 64 output bits as u64. This may not be as efficient
/// as using arrays of u8 on an 8-bit CPU, but hard to tell without testing
#[derive(Default)]
pub struct CmriProcessor {
    input_bits: u64,
    output_bits: u64,
    state: CmriStateMachine,
}

impl CmriProcessor {
    /// Initialise a processor attached to the given UART
    pub fn new(baud: u64) -> Self {
        let ubrr = (CPU_FREQUENCY_HZ / 16 / baud - 1) as u16;

        // Initialise the UART
        // Don't run this when running unit tests
        #[cfg(not(test))]
        serial::Serial::new(ubrr)
            .character_size(serial::CharacterSize::EightBits)
            .mode(serial::Mode::Asynchronous)
            .parity(serial::Parity::Disabled)
            .stop_bits(serial::StopBits::OneBit)
            .configure();

        // todo address filter
        Default::default()
    }

    pub fn process(&mut self) {
        use MessageType::*;
        // Read input chars while they are available
        while let Some(b) = serial::try_receive() {
            if let Ok(RxState::Complete) = self.state.process(b) {
                // got the end of a message; process its contents
                if let Some(t) = self.state.message().message_type {
                    match t {
                        Set => {
                            // copy message bits into local buffer
                        }
                        Poll => {
                            // send a response back with our local input
                            // buffer
                        }
                        _ => {}
                    }
                }
                // Break to allow program to update hardware outputs
                // with new information/pull new sensor data in before
                // next poll
                break;
            }
        }
    }

    pub fn get_bit(&self, bit: u8) -> bool {
        // Ignore overflows
        if bit > OUTPUT_BITS - 1 {
            return false;
        }

        let mask: u64 = 1 << (OUTPUT_BITS - 1 - bit);

        self.output_bits & mask != 0
    }

    pub fn get_byte(&self, byte: u8) -> u8 {
        // ignore overflows
        if byte > OUTPUT_BYTES - 1 {
            return 0;
        }

        self.output_bits.to_be_bytes()[byte as usize]

        //((self.output_bits >> 8*(OUTPUT_BYTES - 1 - byte)) & 0xff) as u8
    }

    pub fn set_bit(bit: u8, state: bool) {
        todo!()
    }

    pub fn set_byte(&mut self, byte: u8, state: u8) {
        // ignore overflows
        if byte > OUTPUT_BYTES - 1 {
            return;
        }

        let mut bytes = self.input_bits.to_be_bytes();
        bytes[byte as usize] = state;
        self.input_bits = u64::from_be_bytes(bytes);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::random;
    use std::eprintln;
    use std::format;
    use std::vec::Vec;

    fn bits(num: u64) -> Vec<bool> {
        let strbits = format!("{:064b}", num);
        strbits
            .chars()
            .map(|c| if c == '0' { false } else { true })
            .collect()
    }

    #[test]
    fn get_bit() {
        let mut p = CmriProcessor::new(9600);
        // 1111 0000 0001 0010 1010 1011 0011 0100
        // 1100 1101 0000 0000 0000 0000 1010 1010
        p.output_bits = 0xf012_ab34_cd00_00aa;

        assert_eq!(p.get_bit(0), true);
        assert_eq!(p.get_bit(1), true);
        assert_eq!(p.get_bit(4), false);
    }

    #[test]
    fn get_bit_random() {
        // Try fetching bits from five random numbers
        let mut p = CmriProcessor::new(9600);

        for _ in 0..5 {
            let number: u64 = random();
            eprintln!("Random number is: {}", number);
            eprintln!("Binary representation: {:064b}", number);
            p.output_bits = number;

            for (n, bit) in bits(number).iter().enumerate() {
                assert_eq!(p.get_bit(n as u8), *bit);
            }
        }
    }

    #[test]
    fn get_byte() {
        let mut p = CmriProcessor::new(9600);
        p.output_bits = 0x1234_5678_90ab_cdef;

        assert_eq!(p.get_byte(0), 0x12);
        assert_eq!(p.get_byte(1), 0x34);

        assert_eq!(p.get_byte(2), 0x56);
        assert_eq!(p.get_byte(3), 0x78);

        assert_eq!(p.get_byte(4), 0x90);
        assert_eq!(p.get_byte(5), 0xab);

        assert_eq!(p.get_byte(6), 0xcd);
        assert_eq!(p.get_byte(7), 0xef);
    }

    #[test]
    fn get_byte_random() {
        let mut p = CmriProcessor::new(9600);
        for _ in 0..5 {
            let number: u64 = random();
            eprintln!("Random number is: {}", number);
            eprintln!("Hex representation: {:16x}", number);
            p.output_bits = number;

            let mut bytes = [0_u8; 8];
            for (n, b) in bytes.iter_mut().enumerate() {
                *b = p.get_byte(n as u8);
            }
            eprintln!("Bytes array: {:?}", bytes);
            let converted = u64::from_be_bytes(bytes);
            eprintln!("Converted: {:x}", converted);

            assert_eq!(converted, number);
        }
    }

    #[test]
    fn set_byte() {
        let mut p = CmriProcessor::new(9600);
        let bytes: [u8; 8] = [12, 34, 45, 67, 78, 89, 123, 43];

        for (n, b) in bytes.iter().enumerate() {
            p.set_byte(n as u8, *b);
        }

        assert_eq!(p.input_bits, u64::from_be_bytes(bytes));
    }

    #[test]
    fn set_byte_random() {
        let mut p = CmriProcessor::new(9600);
        let mut bytes = [0_u8; 8];

        for _ in (0..5) {
            // Pick 8 random bytes
            for (n, b) in bytes.iter_mut().enumerate() {
                *b = random();
                p.set_byte(n as u8, *b);
            }
            eprintln!("Random bytes: {:?}", bytes);

            assert_eq!(p.input_bits, u64::from_be_bytes(bytes));
        }
    }
}
