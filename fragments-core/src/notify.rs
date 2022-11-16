use std::{
    sync::{
        atomic::{
            AtomicBool,
            Ordering::{self, SeqCst},
        },
        Arc,
    },
    task::{Poll, Waker},
};

use futures::Future;
use parking_lot::Mutex;

struct AsyncSignal {
    waker: Mutex<Option<Waker>>,
    woken: AtomicBool,
}

impl AsyncSignal {
    pub fn new() -> Self {
        Self {
            waker: Mutex::new(None),
            woken: AtomicBool::new(false),
        }
    }

    pub fn wake(&self) {
        self.woken.store(true, SeqCst);
        if let Some(waker) = &*self.waker.lock() {
            waker.wake_by_ref()
        }
    }

    pub fn set_waker(&self, waker: Waker) {
        *self.waker.lock() = Some(waker)
    }
}

pub struct NotifyReceiver {
    signal: Arc<AsyncSignal>,
}

impl Future for NotifyReceiver {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self
            .signal
            .woken
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Poll::Ready(())
        } else {
            self.signal.set_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct NotifySender {
    signal: Arc<AsyncSignal>,
}

impl NotifySender {
    pub fn notify(&self) {
        self.signal.wake()
    }
}
