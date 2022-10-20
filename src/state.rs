use std::{
    sync::Arc,
    task::{Poll, Waker},
};

use flax::{entity::EntityKind, Entity, World};
use futures::{Future, StreamExt};
use once_cell::sync::OnceCell;

use crate::{
    error::Error,
    fragment::{Fragment, FragmentState},
};

pub(crate) type Effect = Box<dyn FnMut(&mut State) + Send + Sync>;
pub(crate) type SharedEffect = Arc<dyn Fn(&mut State) + Send + Sync>;

/// The UI state of the world
pub struct State {
    world: World,
    events_tx: flume::Sender<Event>,
    events_rx: Option<flume::Receiver<Event>>,
}

impl State {
    /// Creates a new state
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            world: World::new(),
            events_tx: tx,
            events_rx: Some(rx),
        }
    }

    /// Runs the main update loop and handle state updates.
    ///
    /// Mutating and accessing the state is accomplished internally by the event/poll system.
    pub async fn run(mut self) -> Result<(), Error> {
        let mut events = self
            .events_rx
            .take()
            .expect("State::run called more than once")
            .into_stream();

        // Handle events as they come in
        while let Some(event) = events.next().await {
            match event {
                Event::RunEffect(mut effect) => {
                    effect(&mut self);
                }
                Event::RunSharedEffect(effect) => {
                    effect(&mut self);
                }
                Event::SpawnEntity { data, waker } => {
                    let id = self.world.spawn();
                    data.set(id).unwrap();
                    waker.wake()
                }
            }
        }

        Ok(())
    }

    /// Spawns a new root fragment
    ///
    /// The futures runs until the fragment ends, which may be forever since fragments can enter a
    /// yield-update loop.
    pub fn spawn_fragment(&mut self, frag: impl Fragment) -> impl Future<Output = ()> {
        let id = self.world.spawn();
        let state = FragmentState::new(id, self.handle());
        frag.render(state)
    }

    pub(crate) fn events(&self) -> &flume::Sender<Event> {
        &self.events_tx
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Acquire a handle to the state which can be used to communicate to the state after run
    pub fn handle(&self) -> StateRef {
        StateRef {
            tx: self.events_tx.clone(),
        }
    }
}

/// Cheap to clone handle which allows communication with the UI/fragment state.
#[derive(Debug, Clone)]
pub struct StateRef {
    pub(crate) tx: flume::Sender<Event>,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) enum Event {
    RunEffect(Effect),
    RunSharedEffect(SharedEffect),
    SpawnEntity {
        data: Arc<OnceCell<Entity>>,
        waker: Waker,
    },
}

// Future for asynchronously spawning an entity into the world
pub(crate) struct SpawnEntityFuture {
    tx: flume::Sender<Event>,
    data: Arc<OnceCell<Entity>>,
}

impl Future for SpawnEntityFuture {
    type Output = Entity;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.data.get() {
            Some(&v) => Poll::Ready(v),
            None => {
                self.tx
                    .send(Event::SpawnEntity {
                        waker: cx.waker().clone(),
                        data: self.data.clone(),
                    })
                    .unwrap();

                Poll::Pending
            }
        }
    }
}
