//! Single-core, interrupt-disabled conformance heap. Never resets live storage.
//! Exhaustion is fatal to this firmware run, not a NEPL3 logical Budget stop.
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
};
pub const HEAP_BYTES: usize = 64 * 1024;
#[repr(C, align(16))]
struct State {
    bytes: [u8; HEAP_BYTES],
    next: usize,
}
pub struct Heap(UnsafeCell<State>);
// Only core 0 runs; interrupts remain disabled for the complete test program.
unsafe impl Sync for Heap {}
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // No interrupt, second core or host thread can access this state.
        let state = self.0.get();
        // Do not form &mut State / &mut [u8]: earlier live allocations alias
        // the byte array. Only the separate cursor field is read/written.
        let buffer = unsafe { core::ptr::addr_of_mut!((*state).bytes) }.cast::<u8>();
        let cursor = unsafe { core::ptr::addr_of_mut!((*state).next) };
        let base = buffer as usize;
        let Some(start) = base
            .checked_add(unsafe { cursor.read() })
            .and_then(|n| n.checked_add(layout.align() - 1))
            .map(|n| n & !(layout.align() - 1))
        else {
            return core::ptr::null_mut();
        };
        let Some(end) = start.checked_add(layout.size()) else {
            return core::ptr::null_mut();
        };
        if end > base + HEAP_BYTES {
            return core::ptr::null_mut();
        }
        unsafe {
            cursor.write(end - base);
            buffer.add(start - base)
        }
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
#[global_allocator]
static HEAP: Heap = Heap(UnsafeCell::new(State {
    bytes: [0; HEAP_BYTES],
    next: 0,
}));
