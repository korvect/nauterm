//! Owned Rust wakeup callbacks. Foreign transports receive only an opaque key.
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
};

type Callback = Arc<dyn Fn() + Send + Sync>;

fn callbacks() -> &'static Mutex<HashMap<usize, Callback>> {
    static CALLBACKS: OnceLock<Mutex<HashMap<usize, Callback>>> = OnceLock::new();
    CALLBACKS.get_or_init(Mutex::default)
}

/// Keeps a callback registered. Dropping it prevents new deliveries; a callback
/// already in progress may finish, retaining its own Arc rather than UI memory.
pub struct Subscription(usize);

impl Subscription {
    pub(crate) fn new(callback: impl Fn() + Send + Sync + 'static) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        callbacks().lock().unwrap().insert(id, Arc::new(callback));
        Self(id)
    }

    pub(crate) fn callback(&self) -> crate::pty::WakeupCallback {
        crate::pty::WakeupCallback::new(deliver, self.0 as *mut c_void)
    }
}

extern "C" fn deliver(key: *mut c_void) {
    let callback = callbacks()
        .lock()
        .ok()
        .and_then(|map| map.get(&(key as usize)).cloned());
    if let Some(callback) = callback {
        // A Rust callback must not unwind across a foreign transport boundary.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut map) = callbacks().lock() {
            map.remove(&self.0);
        }
    }
}

impl crate::session::SessionManager {
    pub fn subscribe(
        &mut self,
        id: u64,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Option<Subscription> {
        let subscription = Subscription::new(callback);
        self.set_wakeup_callback(id, Some(subscription.callback()))
            .then_some(subscription)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_transport_key_cannot_reach_dropped_subscription() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let subscription = Subscription::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        let callback = subscription.callback();
        callback.call();
        drop(subscription);
        callback.call();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
