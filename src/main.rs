#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_main]
#![no_std]
#![allow(non_snake_case)]

use panic_semihosting as _;

use cortex_m::{asm::delay, peripheral::DWT};
use cortex_m_semihosting::hprintln;

use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::{
    delay::Delay,
    gpio::*,
    prelude::*,
    usb::{Peripheral, UsbBus, UsbBusType},
};

use crate::midi::{ControlChange, MidiMessage, NoteOff, NoteOn};

use usb_device::bus;
use usb_device::prelude::*;

mod midi;
mod usb_midi;

// SYSCLK = 72MHz --> clock_period = 13.9ns

const VID: u16 = 0x1ACC;
const PID: u16 = 0x3801;

type LED = gpioc::PC13<Output<PushPull>>;

#[rtic::app(device = stm32f1xx_hal::pac, monotonic = rtic::cyccnt::CYCCNT, peripherals = true)]
const APP: () = {
    struct Resources {
        USB_DEV: UsbDevice<'static, UsbBusType>,
        MIDI: usb_midi::MidiClass<'static, UsbBusType>,
        LED: LED,
    }

    #[init()]
    fn init(mut cx: init::Context) -> init::LateResources {
        static mut USB_BUS: Option<bus::UsbBusAllocator<UsbBusType>> = None;

        cx.core.DCB.enable_trace();
        DWT::unlock();
        cx.core.DWT.enable_cycle_counter();

        // Take ownership over the raw flash and rcc devices and convert them into the corresponding
        // HAL structs
        let mut flash = cx.device.FLASH.constrain();
        let mut rcc = cx.device.RCC.constrain();

        // User LED
        let mut gpioc = cx.device.GPIOC.split(&mut rcc.apb2);
        let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

        // Freeze the configuration of all the clocks in the system and store the frozen frequencies
        // in `clocks`
        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(48.mhz())
            .pclk1(24.mhz())
            .freeze(&mut flash.acr);

        assert!(clocks.usbclk_valid());

        let mut gpioa = cx.device.GPIOA.split(&mut rcc.apb2);

        // BluePill board has a pull-up resistor on the D+ line.
        // Pull the D+ pin down to send a RESET condition to the USB bus.
        let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
        usb_dp.set_low().ok();
        delay(clocks.sysclk().0 / 100);

        // USB
        let usb_dm = gpioa.pa11;
        let usb_dp = usb_dp.into_floating_input(&mut gpioa.crh);

        let usb = Peripheral {
            usb: cx.device.USB,
            pin_dm: usb_dm,
            pin_dp: usb_dp,
        };

        *USB_BUS = Some(UsbBus::new(usb));

        let midi = usb_midi::MidiClass::new(USB_BUS.as_ref().unwrap());

        let usb_dev = UsbDeviceBuilder::new(USB_BUS.as_ref().unwrap(), UsbVidPid(VID, PID))
            .manufacturer("jefffm")
            .product("midy-rs")
            .max_power(500)
            .build();

        init::LateResources {
            USB_DEV: usb_dev,
            MIDI: midi,
            LED: led,
        }
    }

    #[idle(resources = [MIDI, LED])]
    fn idle(mut cx: idle::Context) -> ! {
        cx.resources.LED.set_low().unwrap();
        loop {
            hprintln!("idle").unwrap();
            cx.resources.LED.set_low().unwrap();

            // Handle MIDI messages
            let message = cx.resources.MIDI.lock(|m| m.dequeue());

            if let Some(b) = message {
                if let Some(note_on) = NoteOn::from_bytes(b) {
                    cx.resources.LED.set_high().unwrap();
                    hprintln!("note on").unwrap();
                };

                if let Some(note_off) = NoteOff::from_bytes(b) {
                    cx.resources.LED.set_low().unwrap();
                    hprintln!("note off").unwrap();
                };
            }
        }
    }

    #[task(binds = USB_HP_CAN_TX, resources = [USB_DEV, MIDI])]
    fn usb_hp_can_tx(mut cx: usb_hp_can_tx::Context) {
        hprintln!("sending usb").unwrap();
        usb_poll(&mut cx.resources.USB_DEV, &mut cx.resources.MIDI);
    }

    #[task(binds = USB_LP_CAN_RX0, resources = [USB_DEV, MIDI])]
    fn usb_lp_can_rx0(mut cx: usb_lp_can_rx0::Context) {
        hprintln!("receiving usb").unwrap();
        usb_poll(&mut cx.resources.USB_DEV, &mut cx.resources.MIDI);
    }

    extern "C" {
        fn EXTI0();
    }
};

fn usb_poll<B: bus::UsbBus>(
    usb_dev: &mut UsbDevice<'static, B>,
    midi: &mut usb_midi::MidiClass<'static, B>,
) -> bool {
    if !midi.write_queue_is_empty() {
        midi.write_queue_to_host();
    }

    if !usb_dev.poll(&mut [midi]) {
        return false;
    }

    match midi.read_to_queue() {
        Ok(len) if len > 0 => true,
        _ => false,
    }
}
