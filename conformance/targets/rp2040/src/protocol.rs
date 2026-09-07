//! UART0 is configured by the outer board adapter before tests run.
pub fn write(bytes: &[u8]) {
    for &byte in bytes {
        // RP2040 UART0 UARTFR.TXFF and UARTDR. Only this CPU writes UART0.
        unsafe {
            while core::ptr::read_volatile(0x40034018 as *const u32) & (1 << 5) != 0 {}
            core::ptr::write_volatile(0x40034000 as *mut u32, byte as u32);
        }
    }
}
pub fn halt() -> ! {
    loop {
        cortex_m::asm::wfi();
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    write(b"@NEPL3/1 PANIC\n");
    halt()
}
