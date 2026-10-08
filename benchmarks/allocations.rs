//! Separate executable: instrumentation is never linked into timing benchmarks.
use scraper_rs::Document;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
mod common;
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static COUNTS: Cell<(u64, u64, u64, u64)> = const { Cell::new((0, 0, 0, 0)) };
}
struct Counter;
fn record(alloc: u64, realloc: u64, free: u64, bytes: u64) {
    let _ = ACTIVE.try_with(|active| {
        if active.get() {
            let _ = COUNTS.try_with(|c| {
                let old = c.get();
                c.set((old.0 + alloc, old.1 + realloc, old.2 + free, old.3 + bytes));
            });
        }
    });
}
// SAFETY: all pointers/layouts are forwarded unchanged to System. Counters use
// allocation-free thread-local Cells and never touch the allocated memory.
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            record(1, 0, 0, l.size() as u64);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            record(1, 0, 0, l.size() as u64);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        record(0, 0, 1, 0);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let result = unsafe { System.realloc(p, l, n) };
        if !result.is_null() {
            record(0, 1, 0, n as u64);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Counter = Counter;
fn measure<T>(operation: impl FnOnce() -> T) -> (T, (u64, u64, u64, u64)) {
    COUNTS.with(|c| c.set((0, 0, 0, 0)));
    ACTIVE.with(|a| a.set(true));
    let result = operation();
    ACTIVE.with(|a| a.set(false));
    (result, COUNTS.with(Cell::get))
}
fn report(name: &str, fixture: &str, iteration: usize, counts: (u64, u64, u64, u64)) {
    println!(
        "{{\"operation\":\"{name}\",\"fixture\":\"{fixture}\",\"iteration\":{iteration},\"alloc_calls\":{},\"realloc_calls\":{},\"dealloc_calls\":{},\"requested_bytes\":{}}}",
        counts.0, counts.1, counts.2, counts.3
    );
}
fn main() {
    let (v, counts) = measure(|| black_box(vec![1_u8; 123]));
    assert_eq!(counts, (1, 0, 0, 123));
    drop(v);
    for (name, html, matches) in common::selection_fixtures() {
        common::validate_fixture(&html, matches);
        let doc = Document::new(&html, None, false).unwrap();
        drop(doc.select(common::CSS_ITEM).unwrap());
        for iteration in 0..10 {
            let (dom, counts) =
                measure(|| tl::parse(black_box(&html), Default::default()).unwrap());
            report("tl_dom_create_no_drop", name, iteration, counts);
            let (_, counts) = measure(|| drop(dom));
            report("tl_dom_drop", name, iteration, counts);
            let (fresh, counts) = measure(|| Document::new(black_box(&html), None, false).unwrap());
            report("document_create_no_drop", name, iteration, counts);
            let (_, counts) = measure(|| drop(fresh));
            report("document_drop", name, iteration, counts);
            let (results, counts) = measure(|| doc.select(black_box(common::CSS_ITEM)).unwrap());
            assert_eq!(results.len(), matches);
            report("css_select_no_result_drop", name, iteration, counts);
            drop(results);
        }
    }
}
