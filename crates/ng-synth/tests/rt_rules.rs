// <META - ROLE : gate g15 — counting-allocator proof of R-RT-1 and the denormal assertion of R-RT-3. | L1-90>
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use ng_core::{AudioSource, Buses, LaneKind, NoteSink, Slot};
use ng_synth::osc::OscSynth;

const SR: u32 = 44_100;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

/// Tests in this binary share one global allocator: serialize them so another test's
/// incidental allocation cannot land inside the counting window.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::SeqCst) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn render_block(s: &mut OscSynth) {
    let mut dl = [0.0f32; 128];
    let mut dr = [0.0f32; 128];
    let mut sl = [0.0f32; 128];
    let mut sr = [0.0f32; 128];
    let mut buses = Buses {
        dry_l: &mut dl,
        dry_r: &mut dr,
        send_l: &mut sl,
        send_r: &mut sr,
    };
    s.render(&mut buses);
}

#[test]
fn render_loop_allocates_zero_after_warmup() {
    let _guard = SERIAL.lock().unwrap();
    let mut s = OscSynth::new(SR, 16);
    s.set_program(Slot(0), LaneKind::Melodic, 0);
    s.set_program(Slot(1), LaneKind::Drum, 0);
    s.note_on(Slot(0), 60, 100);
    s.note_on(Slot(1), 36, 100);
    // Warm-up: envelope construction, filter coefficient setup, first-block caches.
    for _ in 0..64 {
        render_block(&mut s);
    }
    ALLOCS.store(0, Ordering::SeqCst);
    COUNTING.store(true, Ordering::SeqCst);
    for _ in 0..512 {
        render_block(&mut s);
    }
    COUNTING.store(false, Ordering::SeqCst);
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "render must not allocate");
}

#[test]
fn render_output_never_goes_subnormal() {
    let _guard = SERIAL.lock().unwrap();
    let mut s = OscSynth::new(SR, 8);
    s.set_program(Slot(0), LaneKind::Melodic, 0);
    s.note_on(Slot(0), 69, 100);
    for _ in 0..64 {
        render_block(&mut s);
    }
    s.note_off(Slot(0), 69);
    // Walk the release tail: every produced sample is either exactly zero or a normal float.
    for _ in 0..SR as usize / 128 {
        let mut dl = [0.0f32; 128];
        let mut dr = [0.0f32; 128];
        let mut sl = [0.0f32; 128];
        let mut sr = [0.0f32; 128];
        {
            let mut buses = Buses {
                dry_l: &mut dl,
                dry_r: &mut dr,
                send_l: &mut sl,
                send_r: &mut sr,
            };
            s.render(&mut buses);
        }
        for v in dl.iter().chain(dr.iter()) {
            assert!(
                *v == 0.0 || v.abs() >= f32::MIN_POSITIVE,
                "subnormal in output: {v:e}"
            );
        }
    }
    // And the tail ends in exact zeros, not in denormal dust.
    let mut dl = [1.0f32; 128];
    let mut dr = [1.0f32; 128];
    let mut sl = [1.0f32; 128];
    let mut sr = [1.0f32; 128];
    {
        let mut buses = Buses {
            dry_l: &mut dl,
            dry_r: &mut dr,
            send_l: &mut sl,
            send_r: &mut sr,
        };
        s.render(&mut buses);
    }
    assert!(dl.iter().all(|&x| x == 0.0));
}
