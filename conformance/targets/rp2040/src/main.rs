#![no_std]
#![no_main]
extern crate alloc;
mod allocator;
mod protocol;
mod tests;
use hal::{clocks::Clock, fugit::RateExtU32};
use rp2040_hal as hal;

#[unsafe(link_section = ".boot2")]
#[used]
#[unsafe(no_mangle)]
pub static BOOT2_FIRMWARE: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

#[hal::entry]
fn main() -> ! {
    cortex_m::interrupt::disable();
    let Some(mut pac) = hal::pac::Peripherals::take() else {
        protocol::halt()
    };
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let Ok(clocks) = hal::clocks::init_clocks_and_plls(
        12_000_000,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    ) else {
        protocol::halt()
    };
    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let Ok(uart) = hal::uart::UartPeripheral::new(
        pac.UART0,
        (pins.gpio0.into_function(), pins.gpio1.into_function()),
        &mut pac.RESETS,
    )
    .enable(
        hal::uart::UartConfig::new(
            115200.Hz(),
            hal::uart::DataBits::Eight,
            None,
            hal::uart::StopBits::One,
        ),
        clocks.peripheral_clock.freq(),
    ) else {
        protocol::halt()
    };
    core::mem::forget(uart);
    protocol::write(b"@NEPL3/1 BEGIN\n");
    for (id, test) in tests::CASES {
        protocol::write(if test() {
            b"@NEPL3/1 PASS "
        } else {
            b"@NEPL3/1 FAIL "
        });
        protocol::write(id.as_bytes());
        protocol::write(b"\n");
    }
    protocol::write(b"@NEPL3/1 END\n");
    protocol::halt()
}
