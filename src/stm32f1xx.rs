use cortex_m::asm::delay;
use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::gpio::Floating;
use stm32f1xx_hal::gpio::Input;
use stm32f1xx_hal::usb::Peripheral;

/// Initializes the bluepill usb stack.
/// This will also set the dp line low. To RESET
/// the usb bus
pub fn initialize_usb(
    clocks: &stm32f1xx_hal::rcc::Clocks,
    pa12: stm32f1xx_hal::gpio::gpioa::PA12<Input<Floating>>,
    pa11: stm32f1xx_hal::gpio::gpioa::PA11<Input<Floating>>,
    crh: &mut stm32f1xx_hal::gpio::gpioa::CRH,
    usb: stm32f1xx_hal::stm32::USB,
) -> stm32f1xx_hal::usb::Peripheral {
    // BluePill board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    let mut usb_dp = pa12.into_push_pull_output(crh);
    usb_dp.set_low().unwrap();
    delay(clocks.sysclk().0 / 100);

    let usb_dm = pa11;
    let usb_dp = usb_dp.into_floating_input(crh);

    let usb = Peripheral {
        usb: usb,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };

    usb
}
