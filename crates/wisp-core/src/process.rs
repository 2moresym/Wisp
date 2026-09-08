use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::{Handle, HandleTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Running,
    Exited,
}

pub struct WispProcess {
    handles: HandleTable,
    threads: Mutex<HashMap<Handle, Arc<WispThread>>>,
}

impl WispProcess {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            handles: HandleTable::new(),
            threads: Mutex::new(HashMap::new()),
        })
    }

    pub fn create_thread<F>(self: &Arc<Self>, f: F) -> Result<Handle, std::io::Error>
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = self.handles.reserve();
        let thread = Arc::new(WispThread::spawn(f)?);
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
    fn spawn<F>(f: F) -> Result<Self, std::io::Error>
    where
        F: FnOnce() + Send + 'static,
    {
        let state = Arc::new(Mutex::new(ThreadState::Running));
        let state_for_thread = Arc::clone(&state);
        let join = thread::Builder::new()
            .name("wisp-thread".into())
            .spawn(move || {
                f();
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
