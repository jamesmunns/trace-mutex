use std::{
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex as StdMutex, MutexGuard as StdMutexGuard, PoisonError as StdPoisonError,
        TryLockError as StdTryLockError,
    },
    thread::sleep,
    time::{Duration, Instant},
};

use log::{debug, error, info, trace, warn};

#[cfg(feature = "1_46_0")]
use std::panic::Location;

const DEFAULT_SPIN: usize = 100;
const SPIN_INCREASE: usize = 2;
const DEBUG_THRESHOLD: usize = 50_000;
const INFO_THRESHOLD: usize = 500_000;
const WARN_THRESHOLD: usize = 3_000_000;
const ERROR_THRESHOLD: usize = 60_000_000;

static MUTEX_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct Mutex<T> {
    inner: StdMutex<T>,
    spin_us: AtomicUsize,
    id: usize,
}

pub struct MutexGuard<'a, T> {
    inner: StdMutexGuard<'a, T>,
    id: String,
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        trace!("{} - Released", self.id);
    }
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

    #[cfg_attr(feature = "1_46_0", track_caller)]
    pub fn lock(&self) -> std::result::Result<MutexGuard<T>, StdPoisonError<StdMutexGuard<T>>> {
        let start = Instant::now();
        #[cfg(feature = "1_46_0")]
        let ident = {
            let loc = Location::caller();
            print_id(&loc, self.id)
        };

        #[cfg(not(feature = "1_46_0"))]
        let ident = { print_id(self.id) };

        loop {
            match self.inner.try_lock() {
                Ok(guard) => {
                    self.spin_us.store(DEFAULT_SPIN, Ordering::Release);
                    trace!("{} - Locked", ident);
                    return Ok(MutexGuard {
                        inner: guard,
                        id: ident,
                    });
                }
                Err(StdTryLockError::WouldBlock) => {
                    let spin = loop {
                        let load = self.spin_us.load(Ordering::Acquire);
                        let store = load.saturating_mul(SPIN_INCREASE);
                        match self.spin_us.compare_exchange(
                            load,
                            store,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        ) {
                            Ok(spin) => break spin,
                            Err(_) => {}
                        }
                    };

                    match spin {
                        n if n < DEBUG_THRESHOLD => {}
                        n if n < INFO_THRESHOLD => {
                            debug!("{} - Waiting {:?}", ident, start.elapsed())
                        }
                        n if n < WARN_THRESHOLD => {
                            info!("{} - Waiting {:?}", ident, start.elapsed())
                        }
                        n if n < ERROR_THRESHOLD => {
                            warn!("{} - Waiting {:?}", ident, start.elapsed())
                        }
                        _ => error!("{} - Waiting {:?}", ident, start.elapsed()),
                    }
                    sleep(Duration::from_micros(spin as u64));
                }
                Err(StdTryLockError::Poisoned(p)) => return Err(p),
            }
        }
    }
}

#[cfg(not(feature = "1_46_0"))]
fn print_id(id: usize) -> String {
    format!("Mutex id: {}", id)
}

#[cfg(feature = "1_46_0")]
fn print_id(loc: &Location, id: usize) -> String {
    format!("Lock {} at {}:{}", id, loc.file(), loc.line())
}
