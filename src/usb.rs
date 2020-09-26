use crate::usb_midi::MidiClass;
use cortex_m_semihosting::hprintln;
use usb_device::bus::UsbBus;
use usb_device::bus::UsbBusAllocator;
use usb_device::device::UsbDevice;
use usb_device::device::UsbVidPid;
use usb_device::prelude::UsbDeviceBuilder;

const VID: u16 = 0x1ACC;
const PID: u16 = 0x3801;

/// Configures the usb device as seen by the operating system.
pub fn configure_usb<'a, B: UsbBus>(usb_bus: &'a UsbBusAllocator<B>) -> UsbDevice<'a, B> {
    let usb_vid_pid = UsbVidPid(VID, PID);

    UsbDeviceBuilder::new(usb_bus, usb_vid_pid)
        .manufacturer("jefffm")
        .product("midy-rs")
        .serial_number("1")
        .max_power(500)
        .build()
}

/// Called to process any usb events
/// Note: this needs to be called often,
/// and seemingly always the same way
pub fn usb_poll<B: UsbBus>(
    usb_dev: &mut UsbDevice<'static, B>,
    midi: &mut MidiClass<'static, B>,
) -> bool {
    if !midi.write_queue_is_empty() {
        if let Err(err) = midi.write_queue_to_host() {
            hprintln!("Error writing midi queue to USB bus: {:?}", err).unwrap();
        }
    }

    if !usb_dev.poll(&mut [midi]) {
        return false;
    }

    match midi.read_to_queue() {
        Ok(len) if len > 0 => true,
        _ => false,
    }
}
