use crate::utils::snowflake::SnowflakeGenerator;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, thread_local};

static THREAD_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct ThreadState {
    _worker_id: u64,
    snowflake: SnowflakeGenerator,
}

thread_local! {
    static STATE: RefCell<Option<ThreadState>> = RefCell::new(None);
}

fn thread_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut ThreadState) -> R,
{
    STATE.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            let id = THREAD_COUNTER.fetch_add(1, Ordering::Relaxed) as u64;

            *opt = Some(ThreadState {
                _worker_id: id,
                snowflake: SnowflakeGenerator::new(
                    env::var("SERVER_ID")
                        .unwrap_or("0".to_string())
                        .parse()
                        .expect("SERVER_ID wrong type"),
                    id,
                ),
            });
        }

        let state = opt.as_mut().unwrap();
        f(state)
    })
}

pub fn generate_id() -> i64 {
    thread_state(|st| st.snowflake.generate())
}
