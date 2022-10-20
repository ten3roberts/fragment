use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use async_trait::async_trait;
use flax::{buffer::ComponentBuffer, Component, ComponentValue, Entity};
use futures::future::BoxFuture;

use crate::state::{Event, SharedEffect, State, StateRef};

/// State containing the state for an element "fragment" of the UI state.
pub struct FragmentData {
    id: Entity,
    components: ComponentBuffer,

    sync_effect: SharedEffect,
    state: StateRef,
}

impl FragmentData {
    /// Sets the component value
    pub fn set<T: ComponentValue>(&mut self, component: Component<T>, value: T) -> &mut Self {
        self.components.set(component, value);
        self
    }

    pub(crate) fn flush(&self) {
        self.state
            .tx
            .send(Event::RunSharedEffect(self.sync_effect.clone()))
            .ok();
    }
}

/// Cheaply cloneable reference to a fragment in the tree.
///
/// Allows for sync and async modification.
pub struct FragmentState {
    data: Arc<RwLock<FragmentData>>,
}

impl FragmentState {
    pub(crate) fn new(id: Entity, state: StateRef) -> Self {
        // Make the syncing effect hold a weak reference to the data
        let data = Arc::new_cyclic(|weak: &Weak<RwLock<FragmentData>>| {
            let weak = weak.clone();
            let sync_effect = Arc::new(move |state: &mut State| {
                if let Some(data) = weak.upgrade() {
                    let mut data = data.write().unwrap();
                    let world = state.world_mut();
                    eprintln!("Components: {:?}", data.components);
                    world.set_with(id, &mut data.components).unwrap()
                } else {
                    eprintln!("Data dropped")
                }
            });

            let data = FragmentData {
                id,
                components: ComponentBuffer::new(),
                state,
                sync_effect,
            };

            RwLock::new(data)
        });

        Self { data }
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

#[async_trait]
pub trait Fragment: Send + Sync + 'static {
    /// Renders the component into an entity
    async fn render(self, state: FragmentState);
}
