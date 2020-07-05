use std::{
    sync::{
        Mutex as StdMutex,
        LockResult as StdLockResult,
        TryLockError as StdTryLockError,
        TryLockResult as StdTryLockResult,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
    thread::sleep,
    time::{
        Instant,
        Duration,
    },
};

use log::{trace, debug, warn, error};
use std::panic::Location;

pub use std::sync::MutexGuard as StdMutexGuard;

const DEFAULT_SPIN: usize = 100;
const SPIN_INCREASE: usize = 2;
const TRACE_THRESHOLD: usize = 50_000;
const DEBUG_THRESHOLD: usize = 500_000;
const WARN_THRESHOLD: usize = 3_000_000;
const ERROR_THRESHOLD: usize = 60_000_000;

static MUTEX_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct Mutex<T> {
    inner: StdMutex<T>,
    spin_us: AtomicUsize,
    id: usize,
}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        let id = MUTEX_ID.fetch_add(1, Ordering::AcqRel);
        Self {
            inner: StdMutex::new(data),
            spin_us: AtomicUsize::new(DEFAULT_SPIN),
            id,
        }
    }

    #[track_caller]
    pub fn lock(&self) -> StdLockResult<StdMutexGuard<T>> {
        let start = Instant::now();
        loop {
            match self.inner.try_lock() {
                Ok(guard) => {
                    self.spin_us.store(DEFAULT_SPIN, Ordering::Release);
                    return Ok(guard);
                }
                Err(StdTryLockError::WouldBlock) => {
                    let spin = loop {
                        let load = self.spin_us.load(Ordering::Acquire);
                        let store = load.saturating_mul(SPIN_INCREASE);
                        match self.spin_us.compare_exchange(load, store, Ordering::SeqCst, Ordering::SeqCst) {
                            Ok(spin) => break spin,
                            Err(_) => {}
                        }
                    };
                    let loc = Location::caller();
                    match spin {
                        n if n < TRACE_THRESHOLD => {},
                        n if n < DEBUG_THRESHOLD => trace!("Lock at {}:{} - Waiting {:?}", loc.file(), loc.line(), start.elapsed()),
                        n if n < WARN_THRESHOLD => debug!("Lock at {}:{} - Waiting {:?}", loc.file(), loc.line(), start.elapsed()),
                        n if n < ERROR_THRESHOLD => warn!("Lock at {}:{} - Waiting {:?}", loc.file(), loc.line(), start.elapsed()),
                        _ => error!("Lock at {}:{} - Waiting {:?}", loc.file(), loc.line(), start.elapsed()),
                    }
                    sleep(Duration::from_micros(spin as u64));
                }
                _ => panic!("Mutex id {} poisoned!", self.id),
            }
        }
    }

    pub fn try_lock(&self) -> StdTryLockResult<StdMutexGuard<T>> {
        self.inner.try_lock()
    }
}
