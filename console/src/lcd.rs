// The LCD pushes pixels to the frontend, which does the actual displaying
// of what is on the gameboy screen. The LCD (producer) writes at the rate
// the PPU produces pixels to a circular buffer. It does not wait for or
// track the progress of the frontend (consumer).

use std::sync::{Arc, atomic::{AtomicU8, AtomicUsize, Ordering::{Acquire, Relaxed, Release}}};

const SCREEN_SIZE: usize = 160 * 144;

#[repr(align(64))]
struct CacheLine<T>(T);

#[repr(C)]
struct Shared {
    count: CacheLine<AtomicUsize>,
    lcd_buf: CacheLine<[AtomicU8; SCREEN_SIZE]>,
}

pub struct Producer {
    shared: Arc<Shared>,
    count: usize,
}

pub struct Consumer {
    shared: Arc<Shared>,
    seen_count: usize,
}

pub struct LCD {
    producer: Producer,
}

impl LCD {
    pub fn new(producer: Producer) -> Self {
        LCD {
            producer: producer,
        }
    }

    pub fn push_pixel(&mut self, pixel: u8) {
        self.producer.push(pixel);
    }
}

pub fn make_lcd_channel() -> (Producer, Consumer) {
    let shared = Arc::new(Shared {
        count: CacheLine(AtomicUsize::new(0)),
        lcd_buf: CacheLine([const { AtomicU8::new(0) }; SCREEN_SIZE]),
    });

    (
        Producer { shared: shared.clone(), count: 0, },
        Consumer { shared, seen_count: 0 },
    )
}

impl Producer {
    #[inline(always)]
    pub fn push(&mut self, byte: u8) {
        let index = self.count % SCREEN_SIZE;
        self.count += 1;
        self.shared.lcd_buf.0[index].store(byte, Relaxed);
        self.shared.count.0.store(self.count, Release);
    }
}

impl Consumer {
    // Returns all pixels that were written since last read as well as the index where they start.
    // NOTE: If this is more than SCREEN_SIZE the full screen's buffer is returned instead with start 0.
    pub fn read_new(&mut self) -> (Vec<u8>, usize) {
        let writer_count = self.shared.count.0.load(Acquire);
        // If we are behind more than SCREEN_SIZE, just return the whole screen as it currently is.
        let start = self.seen_count.max(writer_count.saturating_sub(SCREEN_SIZE));

        let out: Vec<u8> = (start..writer_count).map(|i| self.shared.lcd_buf.0[i % SCREEN_SIZE].load(Relaxed)).
            collect();

        self.seen_count = writer_count;
        (out, start % SCREEN_SIZE)
    }
}
