//! The `--json` report is written straight into one string: while it is built,
//! the heap holds the output and a small constant, not a tree of values.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use ebs_orphan_bill::{bill_from_inputs, render, NamedBytes, Options};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = System.realloc(ptr, layout, new_size);
        if !new.is_null() {
            if new_size >= layout.size() {
                let now = LIVE.fetch_add(new_size - layout.size(), Ordering::SeqCst) + new_size
                    - layout.size();
                PEAK.fetch_max(now, Ordering::SeqCst);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::SeqCst);
            }
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn volumes_file(count: usize) -> NamedBytes {
    let mut text = String::from(r#"{"Volumes":["#);
    for n in 0..count {
        if n > 0 {
            text.push(',');
        }
        text.push_str(&format!(
            r#"{{"VolumeId":"vol-{n:08x}","Size":{},"VolumeType":"gp3","Iops":3000,"Throughput":125,"Attachments":[]}}"#,
            n % 500 + 1
        ));
    }
    text.push_str("]}");
    NamedBytes {
        name: "v.json".to_string(),
        bytes: text.into_bytes(),
        strict: true,
    }
}

/// Extra heap, above what was live before, at the peak while `render` runs.
fn peak_of_render(count: usize) -> (usize, usize) {
    let options = Options {
        json: true,
        ..Options::default()
    };
    let bill = bill_from_inputs(&[volumes_file(count)], &options).expect("bill");
    let before = LIVE.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let out = render(&bill, true);
    let peak = PEAK.load(Ordering::SeqCst) - before;
    (peak, out.len())
}

#[test]
fn json_report_peak_is_the_output_plus_a_small_constant() {
    for count in [2000, 4000, 8000] {
        let (peak, len) = peak_of_render(count);
        // The output itself, with at most a doubling of its buffer while it grows.
        assert!(
            peak <= 2 * len + 4096,
            "{count} volumes: peak {peak} bytes for {len} bytes of output"
        );
    }
}
