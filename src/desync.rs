use std::{
    sync::{Arc, Condvar},
    thread,
};

use once_cell::sync::OnceCell;

type Job<T> = Box<dyn Fn(&mut T)>;

pub struct Desync<T> {
    value: T,
    rx: flume::Receiver<Job<T>>,
    handle: DesyncRef<T>,
}

#[derive(Debug)]
pub struct DesyncRef<T> {
    tx: flume::Sender<Job<T>>,
}

impl<T> Clone for DesyncRef<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl<T> Desync<T> {
    pub fn new(value: T) -> Self {
        let (tx, rx) = flume::unbounded();

        Self {
            value,
            rx,
            handle: DesyncRef { tx },
        }
    }

    // pub async fn run(self) -> Self {}

    pub fn handle(&self) -> &DesyncRef<T> {
        &self.handle
    }
}

impl<T> DesyncRef<T> {
    /// Perform an action on the contained value in the background
    fn desync(&self, f: impl Fn(&mut T) + Send + 'static) {
        self.tx.send(Box::new(f)).unwrap();
    }

    /// Perform an action and return the result
    fn sync<R: Send + 'static>(&self, f: impl Fn(&mut T) -> R + Send + 'static) -> R {
        let tid = thread::current();

        let result = Arc::new(OnceCell::new());

        let r = result.clone();
        self.tx
            .send(Box::new(move |v| {
                if r.set(f(v)).is_err() {
                    unreachable!()
                }
                tid.unpark();
            }))
            .unwrap();

        thread::park();

        match Arc::try_unwrap(result).ok().and_then(|v| v.into_inner()) {
            Some(v) => v,
            None => {
                unreachable!()
            }
        }
    }
}
