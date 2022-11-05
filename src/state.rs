use std::{
    iter::once,
    sync::{Arc, Mutex, MutexGuard},
};

use flax::{Entity, World};
use flume::{Receiver, Sender};

use futures::{future::select, try_join, FutureExt};
use slotmap::new_key_type;

use crate::fragment::{Fragment, FragmentFuture, FragmentState};

new_key_type! {
    struct EffectKey;
}

pub(crate) type Effect = Box<dyn FnMut(&mut World) + Send>;

/// The UI state of the world
#[derive(Debug)]
pub struct App {
    world: Arc<Mutex<World>>,
    rx: Receiver<Event>,
    tx: Sender<Event>,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            world: Default::default(),
            rx,
            tx,
        }
    }

    /// Runs the app with the provided root fragment
    pub async fn run(self, root: impl Fragment) -> eyre::Result<()> {
        let rx = self.rx;

        let handle = AppRef {
            world: self.world.clone(),
            tx: self.tx,
        };

        let world = &self.world;

        let handle_events = async move {
            while let Ok(event) = rx.recv_async().await {
                let mut world = world.lock().unwrap();
                for event in once(event).chain(rx.drain()) {
                    println!("Handling event: {event:?}");
                    match event {
                        Event::Exit => return Ok(()),
                        Event::Despawn(id) => {
                            world.despawn(id)?;
                        }
                    }
                }
            }

            Ok::<_, eyre::Report>(())
        };

        let handle_tree = async move {
            let state = FragmentState::spawn(&mut world.lock().unwrap(), handle, None);
            root.render(state).await;
            Ok::<_, eyre::Report>(())
        };

        tokio::select! {
            _ = handle_events => {
                println!("Finished event loop");
            }
            _ = handle_tree => {

            }
        }

        println!("Exiting app");

        Ok::<_, eyre::Report>(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl AppRef {
    /// Spawns a new *root* fragment
    ///
    /// The futures runs until the fragment ends, which may be forever since fragments can enter a
    /// yield-update loop.
    pub fn spawn_fragment(&mut self, frag: impl Fragment) -> FragmentFuture {
        let mut world = self.world();
        let id = world.spawn();
        let state = FragmentState::spawn(&mut world, self.clone(), None);
        FragmentFuture {
            future: frag.render(state),
        }
    }

    pub fn world(&self) -> MutexGuard<World> {
        self.world.lock().unwrap()
    }

    pub fn enqueue(&self, event: Event) -> Result<(), flume::SendError<Event>> {
        self.tx.send(event)
    }
}

/// Cheap to clone handle which allows communication with the UI/fragment state.
#[derive(Debug, Clone)]
pub struct AppRef {
    world: Arc<Mutex<World>>,
    tx: Sender<Event>,
}

#[derive(Debug, Clone)]
pub enum Event {
    Despawn(Entity),
    Exit,
}
