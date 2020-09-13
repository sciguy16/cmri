// Copyright 2020 David Young
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#![feature(llvm_asm)]
#![feature(abi_avr_interrupt)]
#![no_std]
#![no_main]

use ruduino::Pin;
use ruduino::legacy::serial;
use ruduino::modules::Timer16Setup;
use ruduino::modules::{ClockSource16, WaveformGenerationMode16};
use ruduino::cores::atmega328p as avr_core;

use avr_core::{port::B5, Timer16};


const CPU_FREQUENCY_HZ: u64 = 16_000_000;
const BAUD: u64 = 9600;
const UBRR: u16 = (CPU_FREQUENCY_HZ / 16 / BAUD - 1) as u16;

const DESIRED_HZ_TIM1: f64 = 1.0;
const TIM1_PRESCALER: u64 = 1024;
const INTERRUPT_EVERY_1_HZ_1024_PRESCALER: u16 = ((CPU_FREQUENCY_HZ as f64
    / (DESIRED_HZ_TIM1 * TIM1_PRESCALER as f64))
    as u64
    - 1) as u16;

#[no_mangle]
pub extern "C" fn main() -> ! {
    // Onboard LED is called PORTB5

    B5::set_output();
    B5::set_high();


    // Initialise serial port

    serial::Serial::new(UBRR)
        .character_size(serial::CharacterSize::EightBits)
        .mode(serial::Mode::Asynchronous)
        .parity(serial::Parity::Disabled)
        .stop_bits(serial::StopBits::OneBit)
        .configure();

    // initialise a timer
    unsafe { llvm_asm!("CLI") }
    Timer16Setup::<Timer16>::new()
        .waveform_generation_mode(
            WaveformGenerationMode16::ClearOnTimerMatchOutputCompare,
        )
        .clock_source(ClockSource16::Prescale1024)
        .output_compare_1(Some(INTERRUPT_EVERY_1_HZ_1024_PRESCALER))
        .configure();

        // enable interrupts
        //unsafe { llvm_asm!("SEI") };

    loop {
        // Set all pins on PORTB to high.
        //PORTB::set_mask_raw(0xFF);

        //B5::toggle();

        small_delay();

        // Set all pins on PORTB to low.
        //PORTB::unset_mask_raw(0xFF);

        small_delay();

        if let Some(b) = serial::try_receive() {
            serial::transmit(b);
            serial::transmit(b);
        }
    }
}

/// Interrupt handler for the timer
#[no_mangle]
pub extern "avr-interrupt" fn _ivr_timer1_compare_a() {
    B5::toggle();
    serial::transmit('a' as u8);
    unsafe {
        llvm_asm!("nop; nop; nop; nop; nop");
    }
}

/// A small busy loop.
fn small_delay() {
    for _ in 0..400000 {
        unsafe { llvm_asm!("" :::: "volatile") }
    }
}
