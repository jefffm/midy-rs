#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_main]
#![no_std]
#![allow(non_snake_case)]

mod midi;
mod stm32f1xx;
mod usb;
mod usb_midi;

extern crate panic_semihosting;

use crate::midi::{ControlChange, MidiMessage, NoteOff, NoteOn};
use crate::stm32f1xx::initialize_usb;
use crate::usb::{configure_usb, usb_poll};
use crate::usb_midi::MidiClass;
use cortex_m_semihosting::hprintln;
use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::{
    gpio::gpioc::PC13,
    gpio::{Output, PushPull},
    prelude::*,
    usb::{UsbBus, UsbBusType},
};
use usb_device::{bus, prelude::UsbDevice};

#[rtic::app(device = stm32f1xx_hal::pac, monotonic = rtic::cyccnt::CYCCNT, peripherals = true)]
const APP: () = {
    struct Resources {
        usb_dev: UsbDevice<'static, UsbBusType>,
        midi: MidiClass<'static, UsbBusType>,
        led: PC13<Output<PushPull>>,
    }

    #[init()]
    fn init(cx: init::Context) -> init::LateResources {
        static mut USB_BUS: Option<bus::UsbBusAllocator<UsbBusType>> = None;

        // Take ownership over the raw flash and rcc devices and convert them into the corresponding
        // HAL structs
        let mut rcc = cx.device.RCC.constrain();
        let mut flash = cx.device.FLASH.constrain();
        let mut gpioa = cx.device.GPIOA.split(&mut rcc.apb2);
        let mut gpioc = cx.device.GPIOC.split(&mut rcc.apb2);
        let pa12 = gpioa.pa12;
        let pa11 = gpioa.pa11;
        let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        let usb = cx.device.USB;

        led.set_high().unwrap();

        // Freeze the configuration of all the clocks in the system and store the frozen frequencies
        // in `clocks`
        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(48.mhz())
            .pclk1(24.mhz())
            .freeze(&mut flash.acr);

        assert!(clocks.usbclk_valid());

        let usb = initialize_usb(&clocks, pa12, pa11, &mut gpioa.crh, usb);
        *USB_BUS = Some(UsbBus::new(usb));
        let midi = MidiClass::new(USB_BUS.as_ref().unwrap());
        let usb_dev = configure_usb(USB_BUS.as_ref().unwrap());

        init::LateResources {
            usb_dev: usb_dev,
            midi: midi,
            led: led,
        }
    }

    #[idle(resources = [led, midi])]
    fn idle(mut cx: idle::Context) -> ! {
        loop {
            // Handle MIDI messages
            let message = cx.resources.midi.lock(|m| m.dequeue());

            if let Some(b) = message {
                if let Some(note_on) = NoteOn::from_bytes(b) {
                    cx.resources.led.set_low().unwrap();
                };
                if let Some(note_off) = NoteOff::from_bytes(b) {
                    cx.resources.led.set_high().unwrap();
                };
            }
        }
    }

    // Process usb events straight away from High priority interrupts
    #[task(binds = USB_HP_CAN_TX,resources = [usb_dev, midi], priority=3)]
    fn usb_hp_can_tx(mut cx: usb_hp_can_tx::Context) {
        usb_poll(&mut cx.resources.usb_dev, &mut cx.resources.midi);
    }

    // Process usb events straight away from Low priority interrupts
    #[task(binds= USB_LP_CAN_RX0, resources = [usb_dev, midi], priority=3)]
    fn usb_lp_can_rx0(mut cx: usb_lp_can_rx0::Context) {
        usb_poll(&mut cx.resources.usb_dev, &mut cx.resources.midi);
    }

    // Required for software tasks
    extern "C" {
        // Uses the DMA1_CHANNELX interrupts for software
        // task scheduling.
        fn DMA1_CHANNEL1();
        fn DMA1_CHANNEL2();
    }
};
