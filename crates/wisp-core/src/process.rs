use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::{Handle, HandleTable, Peb, Teb, TlsManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Running,
    Exited,
}

pub struct WispProcess {
    handles: HandleTable,
    peb: Arc<Peb>,
    tls: TlsManager,
    threads: Mutex<HashMap<Handle, Arc<WispThread>>>,
}

impl WispProcess {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            handles: HandleTable::new(),
            peb: Arc::new(Peb::new()),
            tls: TlsManager::new(),
            threads: Mutex::new(HashMap::new()),
        })
    }

    #[inline]
    pub fn peb(&self) -> &Arc<Peb> { &self.peb }

    #[inline]
    pub fn tls_alloc(&self) -> Option<usize> { self.tls.alloc() }

    #[inline]
    pub fn tls_free(&self, index: usize) -> bool { self.tls.free(index) }

    #[inline]
    pub fn tls_index_allocated(&self, index: usize) -> bool { self.tls.is_allocated(index) }

    #[inline]
    pub fn tls_get_value(&self, index: usize) -> Option<usize> {
        self.tls.is_allocated(index).then(|| crate::tls::current_get(index))
    }

    #[inline]
    pub fn tls_set_value(&self, index: usize, value: usize) -> bool {
        self.tls.is_allocated(index) && crate::tls::current_set(index, value)
    }

    pub fn create_thread<F>(self: &Arc<Self>, f: F) -> Result<Handle, std::io::Error>
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = self.handles.reserve();
        let thread = Arc::new(WispThread::spawn(f, Arc::clone(&self.peb))?);
        self.threads
            .lock()
            .expect("process thread table poisoned")
            .insert(handle, thread);
        Ok(handle)
    }

    pub fn thread_state(&self, handle: Handle) -> Option<ThreadState> {
        self.threads
            .lock()
            .expect("process thread table poisoned")
            .get(&handle)
            .map(|thread| thread.state())
    }

    pub fn wait_thread(&self, handle: Handle) -> Option<thread::Result<()>> {
        let thread = self
            .threads
            .lock()
            .expect("process thread table poisoned")
            .get(&handle)
            .cloned()?;
        Some(thread.wait())
    }

    pub fn close_thread(&self, handle: Handle) -> bool {
        self.threads
            .lock()
            .expect("process thread table poisoned")
            .remove(&handle)
            .is_some()
    }
}

struct WispThread {
    state: Arc<Mutex<ThreadState>>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl WispThread {
    fn spawn<F>(f: F, peb: Arc<Peb>) -> Result<Self, std::io::Error>
    where
        F: FnOnce() + Send + 'static,
    {
        let state = Arc::new(Mutex::new(ThreadState::Running));
        let state_for_thread = Arc::clone(&state);
        let join = thread::Builder::new()
            .name("wisp-thread".into())
            .spawn(move || {
                let thread_id = unsafe { libc::syscall(libc::SYS_gettid) as u64 };
                let process_id = std::process::id() as u64;
                let mut teb = Teb::new(&peb, process_id, thread_id);

                if let Some((stack_base, stack_limit)) = crate::teb::current_stack_bounds() {
                    teb.set_stack_bounds(stack_base, stack_limit);
                }

                let guard = match teb.install() {
                    Ok(guard) => guard,
                    Err(error) => {
                        eprintln!("wisp: failed to install TEB: {error}");
                        if let Ok(mut state) = state_for_thread.lock() {
                            *state = ThreadState::Exited;
                        }
                        return;
                    }
                };

                f();
                drop(guard);
                if let Ok(mut state) = state_for_thread.lock() {
                    *state = ThreadState::Exited;
                }
            })?;

        Ok(Self {
            state,
            join: Mutex::new(Some(join)),
        })
    }

    fn state(&self) -> ThreadState {
        *self.state.lock().expect("thread state poisoned")
    }

    fn wait(&self) -> thread::Result<()> {
        let join = self
            .join
            .lock()
            .expect("thread join handle poisoned")
            .take();
        match join {
            Some(join) => join.join(),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn thread_bootstraps_teb() {
        let process = WispProcess::new();
        let observed = Arc::new(AtomicBool::new(false));
        let observed_thread = Arc::clone(&observed);
        let peb = Arc::clone(process.peb());

        let handle = process
            .create_thread(move || {
                let teb = crate::teb::current_teb_base();
                let current_peb = crate::teb::current_peb_base();
                if !teb.is_null() && current_peb == peb.as_ptr() {
                    observed_thread.store(true, Ordering::Release);
                }
                thread::sleep(Duration::from_millis(5));
            })
            .expect("thread creation should succeed");

        process
            .wait_thread(handle)
            .expect("thread handle should exist")
            .expect("thread should exit cleanly");
        assert!(observed.load(Ordering::Acquire));
        assert!(process.close_thread(handle));
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn tls_values_are_thread_local() {
        let process = WispProcess::new();
        let index = process.tls_alloc().expect("TLS index should allocate");
        let seen = Arc::new(AtomicBool::new(false));
        let seen_thread = Arc::clone(&seen);

        let handle = process
            .create_thread(move || {
                assert!(crate::tls::current_set(index, 0x1234));
                assert_eq!(crate::tls::current_get(index), 0x1234);
                seen_thread.store(true, Ordering::Release);
            })
            .expect("thread creation should succeed");

        process
            .wait_thread(handle)
            .expect("thread handle should exist")
            .expect("thread should exit cleanly");
        assert!(seen.load(Ordering::Acquire));
        // TLS belongs to each thread; the creating thread's slot remains zero.
        assert_eq!(crate::tls::current_get(index), 0);
        assert!(process.tls_free(index));
    }

    #[test]
    fn thread_lifecycle_is_linux_backed() {
        let process = WispProcess::new();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_thread = Arc::clone(&finished);

        let handle = process
            .create_thread(move || {
                thread::sleep(Duration::from_millis(5));
                finished_thread.store(true, Ordering::Release);
            })
            .expect("thread creation should succeed");

        assert!(matches!(
            process.thread_state(handle),
            Some(ThreadState::Running)
        ));
        process
            .wait_thread(handle)
            .expect("thread handle should exist")
            .expect("thread should exit cleanly");
        assert!(finished.load(Ordering::Acquire));
        assert_eq!(process.thread_state(handle), Some(ThreadState::Exited));
        assert!(process.close_thread(handle));
        assert_eq!(process.thread_state(handle), None);
    }
}
