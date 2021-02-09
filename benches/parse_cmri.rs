use criterion::{criterion_group, criterion_main, Criterion};

use cmri::parser::*;
use cmri::CmriStateMachine;

fn nom_impl(c: &mut Criterion) {
    #[rustfmt::skip]
    let message = [
            CMRI_PREAMBLE_BYTE,
            CMRI_PREAMBLE_BYTE,
            CMRI_START_BYTE,
            0xa2,                               // Address
            MessageType::Init as u8,            // Type
            0x41, 0x41, 0x41, 0x41,             // Message
            CMRI_ESCAPE_BYTE, CMRI_STOP_BYTE,   // Message
            0x42, 0x42, 0x42, 0x42,             // Message
            CMRI_ESCAPE_BYTE, CMRI_ESCAPE_BYTE, // Message
            0x43, 0x43, 0x43, 0x43,             // Message
            CMRI_STOP_BYTE,
    ];

    c.bench_function("parse with nom", |b| b.iter(|| parse_cmri(&message)));
}

fn original_impl(c: &mut Criterion) {
    #[rustfmt::skip]
    let message = [
            CMRI_PREAMBLE_BYTE,
            CMRI_PREAMBLE_BYTE,
            CMRI_START_BYTE,
            0xa2,                               // Address
            MessageType::Init as u8,            // Type
            0x41, 0x41, 0x41, 0x41,             // Message
            CMRI_ESCAPE_BYTE, CMRI_STOP_BYTE,   // Message
            0x42, 0x42, 0x42, 0x42,             // Message
            CMRI_ESCAPE_BYTE, CMRI_ESCAPE_BYTE, // Message
            0x43, 0x43, 0x43, 0x43,             // Message
            CMRI_STOP_BYTE,
    ];

    let mut s = CmriStateMachine::new();

    c.bench_function("parse with original", |b| {
        b.iter(|| {
            for byte in message.iter() {
                s.process(*byte).unwrap();
            }
        })
    });
}

criterion_group!(benches, nom_impl, original_impl);
criterion_main!(benches);
