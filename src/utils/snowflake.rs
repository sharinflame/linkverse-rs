use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EPOCH: u64 = 1_725_513_600_000u64;
const COUNTER_BITS: u64 = 12;
const PID_BITS: u64 = 5;
const SID_BITS: u64 = 5;

#[derive(Debug)]
struct GeneratorState {
    last_ts: u64,
    counter: u64,
}

#[derive(Debug)]
pub struct SnowflakeGenerator {
    state: Mutex<GeneratorState>,
    server_id: u8,
    worker_id: u64,
}

impl SnowflakeGenerator {
    pub fn new(server_id: u8, worker_id: u64) -> Self {
        let max_pid = (1u64 << PID_BITS) - 1;
        let max_sid = (1u64 << SID_BITS) - 1;
        assert!(
            (worker_id as u64) <= max_pid,
            "worker_id {} exceeds max {}",
            worker_id,
            max_pid
        );
        assert!(
            (server_id as u64) <= max_sid,
            "server_id {} exceeds max {}",
            server_id,
            max_sid
        );

        Self {
            state: Mutex::new(GeneratorState {
                last_ts: 0,
                counter: 0,
            }),
            server_id,
            worker_id,
        }
    }

    fn current_time_ms() -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch");
        now.as_millis() as u64
    }

    pub fn generate(&self) -> i64 {
        let seq_mask = (1u64 << COUNTER_BITS) - 1;
        let out_ts: u64;
        let out_counter: u64;

        // We'll loop until we can produce a valid (ts, counter) pair.
        // The loop sometimes drops the lock and sleeps a millisecond to wait next tick.
        loop {
            let mut st = self.state.lock().unwrap();
            let now = Self::current_time_ms().saturating_sub(EPOCH);

            // If clock moved backwards, wait until it catches up.
            if now < st.last_ts {
                drop(st);
                // short sleep, let clock progress; busy-wait would be CPU-heavy
                thread::sleep(Duration::from_millis(1));
                continue;
            }

            if now == st.last_ts {
                // Same millisecond window
                if st.counter < seq_mask {
                    st.counter += 1;
                    out_ts = st.last_ts;
                    out_counter = st.counter;
                    break;
                } else {
                    // sequence overflow in same ms -> wait next millisecond
                    drop(st);
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
            } else {
                // New millisecond: reset counter and advance timestamp
                st.last_ts = now;
                st.counter = 0;
                out_ts = st.last_ts;
                out_counter = st.counter;
                break;
            }
        }

        let output_u64: u64 = (out_ts << (COUNTER_BITS + SID_BITS + PID_BITS))
            | ((self.worker_id & ((1 << PID_BITS) - 1)) << (COUNTER_BITS + SID_BITS))
            | (((self.server_id as u64) & ((1 << SID_BITS) - 1)) << COUNTER_BITS)
            | (out_counter & ((1 << COUNTER_BITS) - 1));

        output_u64.try_into().unwrap()
    }

    pub fn parse(id: u64) -> (f64, u8, u8, u16) {
        let ts = (id >> (COUNTER_BITS + SID_BITS + PID_BITS)) + EPOCH;
        let pid = ((id >> (COUNTER_BITS + SID_BITS)) & ((1 << PID_BITS) - 1)) as u8;
        let sid = ((id >> COUNTER_BITS) & ((1 << SID_BITS) - 1)) as u8;
        let counter = (id & ((1 << COUNTER_BITS) - 1)) as u16;
        (ts as f64 / 1000.0, sid, pid, counter)
    }
}

/// Function to get when id was created in UNIX timestamp
pub fn created_at(id: i64) -> f64 {
    SnowflakeGenerator::parse(id.try_into().unwrap()).0
}
