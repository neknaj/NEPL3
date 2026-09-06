//! Dev-only measurement executable; the production workspace continues to forbid unsafe code.
//! The only unsafe code forwards GlobalAlloc operations to System, with unchanged arguments.
#![deny(unsafe_op_in_unsafe_fn)]

use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Mapping, MappingKind, OriginError, SourceMap},
    source::{SourceId, SourceSnapshot, SourceStore},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct Observed;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: Observed = Observed;

fn count(size: usize) {
    if ACTIVE.load(Ordering::Relaxed) {
        ALLOCATED.fetch_add(size, Ordering::Relaxed);
    }
}

// SAFETY: Each allocation/deallocation is delegated to the same System allocator with
// unchanged pointer/layout arguments. The observation uses only nonallocating atomics.
unsafe impl GlobalAlloc for Observed {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: GlobalAlloc's caller supplied a valid Layout; delegation preserves it.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: Same allocator and valid Layout as requested by the caller.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The caller's pointer/layout pair came from this System-backed allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size);
        // SAFETY: The caller's allocation and new size satisfy GlobalAlloc::realloc.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

fn measure<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATED.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
    let result = operation();
    ACTIVE.store(false, Ordering::Relaxed);
    (result, ALLOCATED.load(Ordering::Relaxed))
}

fn main() -> Result<(), String> {
    // Prove the wrapper is active; keep the allocation live until after measurement.
    let (control, observed) = measure(|| black_box(vec![7_u8; black_box(4096)]));
    if observed < 4096 {
        return Err(format!("allocator positive control failed: {observed}"));
    }
    drop(control);
    println!("positive control: observed {observed} allocated bytes");

    let mut setup = Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    });
    let original = SourceSnapshot::new(
        SourceId("x".repeat(100_000)),
        0,
        "memory:original".into(),
        b"a".to_vec(),
        &mut setup,
    )
    .map_err(|error| format!("{error:?}"))?;
    let generated = SourceSnapshot::new(
        SourceId("generated".into()),
        0,
        "memory:generated".into(),
        b"a".to_vec(),
        &mut setup,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mapping = Mapping {
        source: original.span(0, 1).map_err(|error| format!("{error:?}"))?,
        target: generated.span(0, 1).map_err(|error| format!("{error:?}"))?,
        kind: MappingKind::Exact,
    };
    let mut sources = SourceStore::default();
    sources
        .insert(original)
        .map_err(|error| format!("{error:?}"))?;
    sources
        .insert(generated)
        .map_err(|error| format!("{error:?}"))?;
    let mut maps = SourceMap::default();
    let mut limits = setup.limits();
    limits.allocation_units = 0;
    let mut operation = Budget::new(limits);
    // Input construction, diagnostics/printing and returned-value cleanup are outside the window.
    let (result, allocated) = measure(|| maps.insert(mapping, &sources, &mut operation));
    println!("SourceMap::insert: {result:?}; observed {allocated} allocated bytes");
    if result != Err(OriginError::Stopped(StopReason::AllocationLimit)) || allocated != 0 {
        return Err("R020: zero allocation budget allowed an allocation before stopping".into());
    }
    println!("allocation regression passed");
    Ok(())
}
