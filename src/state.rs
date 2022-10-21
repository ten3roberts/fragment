use std::{
    f32::consts::E,
    sync::Arc,
    task::{Poll, Waker},
};

use flax::{buffer::ComponentBuffer, child_of, Entity, World};
use futures::{channel::oneshot, future::BoxFuture, Future, FutureExt, StreamExt};
use manual_future::{ManualFuture, ManualFutureCompleter};
use once_cell::sync::OnceCell;
use slotmap::{new_key_type, SlotMap};

use crate::{
    components::attached,
    error::Error,
    fragment::{self, Fragment, FragmentFuture, FragmentState},
};

new_key_type! {
    struct EffectKey;
}

/// Used to call an effect.
///
/// Removes the effect from the state on drop.
#[derive(Debug)]
pub struct EffectHandle {
    key: EffectKey,
    state: StateRef,
}

impl EffectHandle {
    pub fn schedule(&self) {
        self.state.run_effect(self.key)
    }
}

impl Drop for EffectHandle {
    fn drop(&mut self) {
        self.state.remove_effect(self.key)
    }
}

pub(crate) type Effect = Box<dyn FnMut(&mut World) + Send>;

/// The UI state of the world
pub struct State {
    effects: SlotMap<EffectKey, Effect>,
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
            effects: SlotMap::with_key(),
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
                Event::Schedule(effect) => {
                    effect(&mut self);
                }
                Event::ScheduleFut(effect) => {
                    effect(&mut self).await;
                }
                Event::SpawnEntity { parent, completer } => {
                    // let mut entity = Entity::builder();

                    // if let Some(parent) = parent {
                    //     assert!(self.world.is_alive(parent), "Parent is not alive");
                    //     entity.set(attached(), (parent));
                    // }

                    // completer.complete(id).await
                }
                Event::CreateEffect { effect, tx } => {
                    let handle = self.create_effect(effect);
                    tx.send(handle).ok();
                }
                Event::RunEffect(key) => {
                    let effect = self.effects.get_mut(key).expect("Effect does not exist");
                    effect(&mut self.world)
                }
                Event::RemoveEffect(_) => todo!(),
            }
        }

        Ok(())
    }

    /// Spawns a new root fragment
    ///
    /// The futures runs until the fragment ends, which may be forever since fragments can enter a
    /// yield-update loop.
    pub fn spawn_fragment(&mut self, frag: impl Fragment) -> FragmentFuture {
        let id = self.world.spawn();
        let state = FragmentState::spawn(self, None);
        FragmentFuture::new(self.handle(), frag.render(state))
    }

    fn events(&self) -> &flume::Sender<Event> {
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

    pub(crate) fn create_effect(&mut self, f: Effect) -> EffectHandle {
        EffectHandle {
            key: self.effects.insert(f),
            state: self.handle(),
        }
    }
}

/// Cheap to clone handle which allows communication with the UI/fragment state.
#[derive(Debug, Clone)]
pub struct StateRef {
    tx: flume::Sender<Event>,
}

impl StateRef {
    /// Schedule a function to run on the state.
    ///
    /// Returns immediately
    pub(crate) fn schedule(&self, f: impl FnOnce(&mut State) + Send + 'static) {
        self.tx.send(Event::Schedule(Box::new(f))).ok();
    }

    /// Schedule a function to run on the state, returning the result asynchronously
    pub(crate) async fn schedule_async<
        T: Send + Unpin + 'static,
        F: FnOnce(&mut State) -> T + Send + 'static,
    >(
        &self,
        f: F,
    ) -> T {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(Event::Schedule(Box::new(|state| {
                let value = f(state);
                // Ignore if nobody is listening
                tx.send(value).ok();
            })))
            .unwrap();

        rx.await.unwrap()
    }

    // Use [`EffectHandle`]
    fn remove_effect(&self, key: EffectKey) {
        self.tx.send(Event::RemoveEffect(key)).ok();
    }

    fn run_effect(&self, key: EffectKey) {
        self.tx.send(Event::RunEffect(key)).unwrap()
    }

    /// Creates a new effect which can be run on the state later
    pub async fn create_effect(&self, f: impl FnMut(&mut World) + Send + 'static) -> EffectHandle {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(Event::CreateEffect {
                effect: Box::new(f),
                tx,
            })
            .unwrap();

        rx.await.unwrap()
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

enum Event {
    CreateEffect {
        effect: Effect,
        tx: oneshot::Sender<EffectHandle>,
    },
    Schedule(Box<dyn FnOnce(&mut State) + Send>),
    ScheduleFut(Box<dyn FnOnce(&mut State) -> BoxFuture<'static, ()> + Send>),
    RunEffect(EffectKey),
    RemoveEffect(EffectKey),
    SpawnEntity {
        parent: Option<Entity>,
        completer: ManualFutureCompleter<Entity>,
    },
}
