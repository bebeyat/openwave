//! The thread-safe store handle.
//!
//! `update()` only swaps the in-memory value and wakes a background writer; serialization, fsync
//! and rename happen on that thread, at most once per [`DEBOUNCE`] no matter how many updates
//! arrive. The in-memory value is authoritative. `flush()` writes synchronously (used on exit and
//! by tests). A store whose file could not be read is *read-only*: it shows defaults, refuses
//! every update with [`StoreError::ReadOnly`] and never touches the file.
use super::persist;
use super::versioned::{self, LoadReport, StoreError, Versioned};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
    },
    thread::JoinHandle,
    time::Duration,
};

/// Updates arriving within this window are written together.
pub const DEBOUNCE: Duration = Duration::from_millis(300);
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(3);

const NORMAL: u8 = 0;
const READ_ONLY: u8 = 1;
const HALTED: u8 = 2;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct Sched {
    dirty: bool,
    stop: bool,
    started: bool,
}

#[derive(Default)]
struct Io {
    written_gen: u64,
    /// Last bytes written per file (empty = removed), to skip files whose content did not change.
    last: HashMap<&'static str, Vec<u8>>,
    failed: bool,
}

struct Inner<T: Versioned> {
    dir: PathBuf,
    state: Mutex<Arc<T>>,
    report: Mutex<LoadReport>,
    mode: AtomicU8,
    generation: AtomicU64,
    sched: Mutex<Sched>,
    wake: Condvar,
    io: Mutex<Io>,
}

pub struct Store<T: Versioned> {
    inner: Arc<Inner<T>>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

impl<T: Versioned> Store<T> {
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let (value, report) = versioned::load::<T>(&dir);
        let mode = if report.read_only { READ_ONLY } else { NORMAL };
        Self {
            inner: Arc::new(Inner {
                dir,
                state: Mutex::new(Arc::new(value)),
                report: Mutex::new(report),
                mode: AtomicU8::new(mode),
                generation: AtomicU64::new(0),
                sched: Mutex::default(),
                wake: Condvar::new(),
                io: Mutex::default(),
            }),
            writer: Mutex::new(None),
        }
    }

    pub fn report(&self) -> LoadReport {
        lock(&self.inner.report).clone()
    }

    /// Current value; clone-free snapshot that stays valid while others update.
    pub fn get(&self) -> Arc<T> {
        lock(&self.inner.state).clone()
    }

    /// The file could not be read: changes are refused and nothing is written.
    pub fn is_read_only(&self) -> bool {
        self.inner.mode.load(Ordering::SeqCst) == READ_ONLY
    }

    /// Replace the value with `f(current)` and return the new snapshot. The write happens soon
    /// on the background thread; the in-memory value is updated even if a write later fails (a
    /// failed earlier write is reported here until one succeeds). Read-only stores refuse.
    pub fn update(&self, f: impl FnOnce(&T) -> T) -> Result<Arc<T>, StoreError> {
        match self.inner.mode.load(Ordering::SeqCst) {
            READ_ONLY => return Err(StoreError::ReadOnly),
            HALTED => return Ok(self.get()),
            _ => {}
        }
        let next = {
            let mut guard = lock(&self.inner.state);
            let next = Arc::new(f(&guard).sanitize());
            *guard = next.clone();
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
            next
        };
        self.schedule();
        if lock(&self.inner.io).failed {
            return Err(StoreError::Write("previous save failed".into()));
        }
        Ok(next)
    }

    fn schedule(&self) {
        {
            let mut s = lock(&self.inner.sched);
            s.dirty = true;
            if s.stop {
                return;
            }
            if !s.started {
                s.started = true;
                let inner = self.inner.clone();
                let handle = std::thread::Builder::new()
                    .name("openwave-store-writer".into())
                    .spawn(move || inner.writer_loop());
                *lock(&self.writer) = handle.ok();
            }
        }
        self.inner.wake.notify_all();
    }

    /// Write pending changes now, on the calling thread.
    pub fn flush(&self) -> Result<(), StoreError> {
        self.inner.write_now()
    }

    /// If the store is read-only because the file was unreadable, try loading it again (the
    /// antivirus scan may be over). Returns true when the store is writable afterwards.
    pub fn try_recover(&self) -> bool {
        if !self.is_read_only() {
            return true;
        }
        let (value, report) = versioned::load::<T>(&self.inner.dir);
        if report.read_only {
            return false;
        }
        *lock(&self.inner.state) = Arc::new(value);
        *lock(&self.inner.report) = report;
        self.inner.mode.store(NORMAL, Ordering::SeqCst);
        true
    }

    /// Stop the writer, drop the in-memory data and refuse to ever write again (used by "delete
    /// all my data" so nothing resurrects the files).
    pub fn halt(&self) {
        self.inner.mode.store(HALTED, Ordering::SeqCst);
        self.stop_writer();
        *lock(&self.inner.state) = Arc::new(T::default().sanitize());
    }

    fn stop_writer(&self) {
        lock(&self.inner.sched).stop = true;
        self.inner.wake.notify_all();
        if let Some(h) = lock(&self.writer).take() {
            let _ = h.join();
        }
    }
}

impl<T: Versioned> Drop for Store<T> {
    fn drop(&mut self) {
        self.stop_writer();
        let _ = self.inner.write_now();
    }
}

impl<T: Versioned> Inner<T> {
    fn writer_loop(&self) {
        let mut sched = lock(&self.sched);
        loop {
            while !sched.dirty && !sched.stop {
                sched = self
                    .wake
                    .wait(sched)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if sched.stop {
                return;
            }
            // Let a burst of updates settle; `stop` cuts the wait short.
            let (guard, _) = self
                .wake
                .wait_timeout_while(sched, DEBOUNCE, |s| !s.stop)
                .unwrap_or_else(PoisonError::into_inner);
            sched = guard;
            if sched.stop {
                return;
            }
            sched.dirty = false;
            drop(sched);
            let failed = self.write_now().is_err();
            sched = lock(&self.sched);
            if failed && !sched.stop {
                sched.dirty = true;
                let (guard, _) = self
                    .wake
                    .wait_timeout_while(sched, RETRY_AFTER_FAILURE, |s| !s.stop)
                    .unwrap_or_else(PoisonError::into_inner);
                sched = guard;
            }
        }
    }

    /// Serialize the latest value and write the files whose content changed.
    fn write_now(&self) -> Result<(), StoreError> {
        let mut io = lock(&self.io);
        match self.mode.load(Ordering::SeqCst) {
            READ_ONLY => return Err(StoreError::ReadOnly),
            HALTED => return Ok(()),
            _ => {}
        }
        let (value, generation) = {
            let state = lock(&self.state);
            (state.clone(), self.generation.load(Ordering::SeqCst))
        };
        if io.written_gen == generation {
            return Ok(());
        }
        let result = self.write_files(&mut io, &value);
        io.failed = result.is_err();
        if result.is_ok() {
            io.written_gen = generation;
        }
        result
    }

    fn write_files(&self, io: &mut Io, value: &T) -> Result<(), StoreError> {
        let mut files: Vec<(&'static str, Option<Vec<u8>>)> = Vec::new();
        // Sidecars first: a crash in between must not lose data that moved out of the main file.
        for car in value.sidecars() {
            let bytes = match car.value {
                Some(v) => {
                    Some(serde_json::to_vec(&v).map_err(|e| StoreError::Serialize(e.to_string()))?)
                }
                None => None,
            };
            files.push((car.file, bytes));
        }
        files.push((T::FILE, Some(versioned::encode(value)?)));
        for (name, bytes) in files {
            let unchanged = match &bytes {
                Some(b) => io.last.get(name) == Some(b),
                None => io.last.get(name).is_some_and(Vec::is_empty),
            };
            if unchanged {
                continue;
            }
            match &bytes {
                Some(b) => persist::write_atomic(&self.dir, name, b)
                    .map_err(|e| StoreError::Write(e.to_string()))?,
                None => match std::fs::remove_file(self.dir.join(name)) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(StoreError::Write(e.to_string())),
                },
            }
            io.last.insert(name, bytes.unwrap_or_default());
        }
        Ok(())
    }
}
