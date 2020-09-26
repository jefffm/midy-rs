use crate::usb_midi::MidiClass;
use usb_device::bus::UsbBus;
use usb_device::bus::UsbBusAllocator;
use usb_device::device::UsbDevice;
use usb_device::device::UsbVidPid;
use usb_device::prelude::UsbDeviceBuilder;
use usbd_midi::data::usb::constants::USB_CLASS_NONE;

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
        .device_class(USB_CLASS_NONE)
        .build()
}

/// Called to process any usb events
/// Note: this needs to be called often,
/// and seemingly always the same way
pub fn usb_poll<B: UsbBus>(usb_dev: &mut UsbDevice<'static, B>, midi: &mut MidiClass<'static, B>) {
    if !usb_dev.poll(&mut [midi]) {
        return;
    }
}
