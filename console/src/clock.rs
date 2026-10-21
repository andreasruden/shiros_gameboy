// This module corresponds to the purpose of the master clock of the gameboy. But rather than being a clock,
// it's a speed altering module, to ensure we are not running too far ahead in the emulation compared to what
// the real clock's pacing would be.

use std::time::{Duration, Instant};

const TICKS_PER_SECOND: u64 = 4_194_304;  // 2^22
const TICKS_PER_SYNC: u64 = TICKS_PER_SECOND / 512;
const TIME_PER_SYNC: Duration = Duration::from_nanos(TICKS_PER_SYNC * 1_000_000_000 / TICKS_PER_SECOND);
const MAX_AHEAD: Duration = Duration::from_millis(1);
const MAX_BEHIND: Duration = Duration::from_millis(10);

pub struct Clock {
    pub total_ticks: u64,
    pub next_sync: u64,
    pub sync_expected_time: Instant,
}

impl Clock {
    pub fn new() -> Self {
        Clock {
            total_ticks: 0,
            next_sync: 0,
            sync_expected_time: Instant::now(),
        }
    }

    pub fn advance(&mut self, ticks: u8) {
        self.total_ticks += ticks as u64;
        if self.total_ticks < self.next_sync {
            return;
        }

        let now = Instant::now();
        let ahead = self.sync_expected_time.saturating_duration_since(now);
        let behind = now.saturating_duration_since(self.sync_expected_time);

        if ahead > MAX_AHEAD {
            std::thread::sleep(ahead);
        } else if behind > MAX_BEHIND {
            // Reset time rather than try to catch up
            self.sync_expected_time = now;
        }
        self.next_sync += TICKS_PER_SYNC;
        self.sync_expected_time += TIME_PER_SYNC;
    }
}
