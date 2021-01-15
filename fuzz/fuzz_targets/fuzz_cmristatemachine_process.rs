#![no_main]
use libfuzzer_sys::fuzz_target;

use cmri::CmriStateMachine;

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here
    let mut state_machine = CmriStateMachine::new();
    for byte in data {
        state_machine.process(*byte);
    }
});
