use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use async_trait::async_trait;
use flax::{buffer::ComponentBuffer, Component, ComponentValue, Entity, World};
use futures::{future::BoxFuture, Future};

use crate::{
    components::{attached, fragment},
    state::{EffectHandle, State, StateRef},
};

/// State containing the state for an element "fragment" of the UI state.
pub struct FragmentData {
    id: Entity,
    components: ComponentBuffer,
    effect: EffectHandle,
}

impl FragmentData {
    /// Sets the component value
    pub fn set<T: ComponentValue>(&mut self, component: Component<T>, value: T) -> &mut Self {
        self.components.set(component, value);
        self
    }
    pub(crate) fn flush(&self) {
        self.effect.schedule()
    }
}

/// Holds the state for a fragment in the tree.
///
/// Allows for sync and async modification.
#[derive(Clone)]
pub struct FragmentState {
    data: Arc<RwLock<FragmentData>>,
    state: StateRef,
    id: Entity,
}

impl FragmentState {
    pub(crate) fn spawn(state: &mut State, parent: Option<Entity>) -> Self {
        let mut components = ComponentBuffer::new();
        components.set(fragment(), ());
        if let Some(parent) = parent {
            components.set(attached(), parent);
        }

        let id = state.world_mut().spawn_with(&mut components);
        assert_eq!(components.components().count(), 0);

        let data = Arc::new_cyclic(|weak: &Weak<RwLock<FragmentData>>| {
            let weak = weak.clone();
            let effect = Box::new(move |world: &mut World| {
                if let Some(data) = weak.upgrade() {
                    let mut data = data.write().unwrap();
                    eprintln!("Components: {:?}", data.components);
                    world.set_with(id, &mut data.components).unwrap();
                }
            });

            let effect = state.create_effect(effect);
            RwLock::new(FragmentData {
                id,
                components,
                effect,
            })
        });

        FragmentState {
            data,
            id,
            state: state.handle(),
        }
    }

    pub(crate) async fn new(state: StateRef, parent: Option<Entity>) -> Self {
        state
            .schedule_async(move |state| Self::spawn(state, parent))
            .await
    }

    /// Attaches a new child fragment under the current fragment.
    ///
    /// The returned value must be polled to advance the fragment.
    pub fn attach(&self, frag: impl Fragment) -> FragmentFuture {
        let state = self.state.clone();
        let parent = self.id;
        let s = state.clone();

        let future = Box::pin(async move {
            let state = Self::new(s, Some(parent)).await;
            frag.render(state).await
        });

        FragmentFuture { state, future }
    }

    /// Access the contents of the fragment
    pub fn read(&self) -> FragmentReadGuard {
        FragmentReadGuard {
            inner: self.data.read().unwrap(),
        }
    }

    /// Access and modify the contents of the fragment.
    ///
    /// Blocks until a write guard is available.
    pub fn write(&self) -> FragmentWriteGuard {
        FragmentWriteGuard {
            inner: self.data.write().unwrap(),
        }
    }
}

pub struct FragmentReadGuard<'a> {
    inner: RwLockReadGuard<'a, FragmentData>,
}

impl<'a> std::ops::Deref for FragmentReadGuard<'a> {
    type Target = FragmentData;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct FragmentWriteGuard<'a> {
    inner: RwLockWriteGuard<'a, FragmentData>,
}

impl<'a> Drop for FragmentWriteGuard<'a> {
    fn drop(&mut self) {
        // Make sure to sync the inner values
        self.inner.flush()
    }
}

impl<'a> std::ops::Deref for FragmentWriteGuard<'a> {
    type Target = FragmentData;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> std::ops::DerefMut for FragmentWriteGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
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
pub struct FragmentFuture {
    state: StateRef,
    future: BoxFuture<'static, ()>,
}

impl FragmentFuture {
    pub(crate) fn new(state: StateRef, future: BoxFuture<'static, ()>) -> Self {
        Self { state, future }
    }
}

impl Future for FragmentFuture {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}
