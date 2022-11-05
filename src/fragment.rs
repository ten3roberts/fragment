use std::sync::{Arc, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use async_trait::async_trait;
use flax::{
    buffer::ComponentBuffer, Component, ComponentValue, Entity, EntityBuilder, EntityRefMut, World,
};
use futures::{future::BoxFuture, Future, FutureExt};

use crate::{
    components::{attached, fragment},
    state::{AppRef, Event},
};

/// Holds the state for a fragment in the tree.
pub struct FragmentState {
    id: Entity,
    state: AppRef,
    buffer: ComponentBuffer,
}

impl FragmentState {
    pub(crate) fn spawn(world: &mut World, state: AppRef, parent: Option<Entity>) -> Self {
        let mut buffer = ComponentBuffer::new();
        buffer.set(fragment(), ());

        if let Some(parent) = parent {
            buffer.set(attached(), parent);
        }

        let id = world.spawn_with(&mut buffer);

        FragmentState { id, state, buffer }
    }

    /// Access and modify the contents of the fragment.
    ///
    /// Blocks until the world is available.
    pub fn lock(&self) -> FragmentGuard {
        FragmentGuard {
            world: self.state.world(),
            fragment: self,
        }
    }

    pub fn app(&self) -> &AppRef {
        &self.state
    }
}

pub struct FragmentGuard<'a> {
    world: MutexGuard<'a, World>,
    fragment: &'a FragmentState,
}

impl<'a> FragmentGuard<'a> {
    /// Set a component for the entity
    pub fn set<T: ComponentValue>(&mut self, component: Component<T>, value: T) -> &mut Self {
        self.world.set(self.fragment.id, component, value).unwrap();
        self
    }

    /// Attaches a new child fragment under the current fragment.
    ///
    /// The returned value must be polled to advance the fragment.
    pub fn attach(&mut self, frag: impl Fragment) -> FragmentFuture {
        let state = self.fragment.state.clone();
        let parent = self.fragment.id;
        let state = FragmentState::spawn(&mut self.world, state, Some(parent));

        let future = Box::pin(async move { frag.render(state).await });

        FragmentFuture { future }
    }
}

/// Represents a component of the UI tree
#[async_trait]
pub trait Fragment: Send + Sync + 'static {
    /// Renders the component into an entity
    async fn render(self, state: FragmentState);
}

/// A handle to the spawned fragment.
///
/// Polling this advances the fragment.
///
/// Dropping the future causes the fragment subtree to be despawned.
#[must_use]
pub struct FragmentFuture {
    pub(crate) future: BoxFuture<'static, ()>,
}

impl Drop for FragmentState {
    fn drop(&mut self) {
        self.state.enqueue(Event::Despawn(self.id)).ok();
    }
}

impl FragmentFuture {
    pub(crate) fn new(state: AppRef, future: BoxFuture<'static, ()>) -> Self {
        Self { future }
    }
}

impl Future for FragmentFuture {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.future.poll_unpin(cx)
    }
}
