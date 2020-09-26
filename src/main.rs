#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_main]
#![no_std]
#![allow(non_snake_case)]

mod state;
mod stm32f1xx;
mod usb;

extern crate panic_semihosting;

use crate::state::midi_events;
use crate::state::{ApplicationState, Button, Message};
use crate::stm32f1xx::initialize_usb;
use crate::usb::{configure_usb, usb_poll};
use cortex_m::{asm::delay, peripheral::DWT};
use cortex_m_semihosting::hprintln;
use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::{
    delay::Delay,
    gpio::gpioc::PC13,
    gpio::{Output, PushPull},
    prelude::*,
    usb::{Peripheral, UsbBus, UsbBusType},
};
use usb_device::{
    bus,
    prelude::{UsbDevice, UsbDeviceState},
};
use usbd_midi::{
    data::usb_midi::usb_midi_event_packet::UsbMidiEventPacket, midi_device::MidiClass,
};

#[rtic::app(device = stm32f1xx_hal::pac, monotonic = rtic::cyccnt::CYCCNT, peripherals = true)]
const APP: () = {
    struct Resources {
        usb_dev: UsbDevice<'static, UsbBusType>,
        midi: MidiClass<'static, UsbBusType>,
        led: PC13<Output<PushPull>>,
        state: ApplicationState,
    }

    #[init()]
    fn init(mut cx: init::Context) -> init::LateResources {
        static mut USB_BUS: Option<bus::UsbBusAllocator<UsbBusType>> = None;

        // Take ownership over the raw flash and rcc devices and convert them into the corresponding
        // HAL structs
        let mut rcc = cx.device.RCC.constrain();
        let mut flash = cx.device.FLASH.constrain();
        let mut gpioa = cx.device.GPIOA.split(&mut rcc.apb2);
        let mut gpiob = cx.device.GPIOB.split(&mut rcc.apb2);
        let mut gpioc = cx.device.GPIOC.split(&mut rcc.apb2);
        let pa12 = gpioa.pa12;
        let pa11 = gpioa.pa11;
        let led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        let usb = cx.device.USB;

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
            state: ApplicationState::init(),
        }
    }

    #[task( spawn = [send_midi],
            resources = [state],
            priority = 1,
            capacity = 5)]
    fn update(cx: update::Context, message: Message) {
        let old = cx.resources.state.clone();
        ApplicationState::update(&mut *cx.resources.state, message);
        let mut effects = midi_events(&old, cx.resources.state);
        let effect = effects.next();

        match effect {
            Some(midi) => {
                let _ = cx.spawn.send_midi(midi);
            }
            _ => (),
        }
    }

    /// Sends a midi message over the usb bus
    /// Note: this runs at a lower priority than the usb bus
    /// and will eat messages if the bus is not configured yet
    #[task(priority=2, resources = [usb_dev,midi])]
    fn send_midi(cx: send_midi::Context, message: UsbMidiEventPacket) {
        let mut midi = cx.resources.midi;
        let mut usb_dev = cx.resources.usb_dev;

        // Lock this so USB interrupts don't take over
        // Ideally we may be able to better determine this, so that
        // it doesn't need to be locked
        usb_dev.lock(|usb_dev| {
            if usb_dev.state() == UsbDeviceState::Configured {
                midi.lock(|midi| {
                    let _ = midi.send_message(message);
                })
            }
        });
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
