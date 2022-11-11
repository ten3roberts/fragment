use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use flax::{child_of, Component, ComponentValue, Entity, World};
use futures::Future;
use slotmap::secondary::Entry;

use crate::{app::AppRef, components::widget};

#[async_trait]
/// Represents a widget which can be rendered into a fragment of the UI tree.
///
/// Widgets can optionally return a value, which can be used for Input fields or alike.
pub trait Widget {
    type Output;
    async fn render(self, fragment: &mut Fragment) -> Self::Output;
}

pub struct Fragment {
    id: Entity,
    app: AppRef,
}

impl Fragment {
    pub(crate) fn spawn(app: AppRef, parent: Option<Entity>) -> Fragment {
        let mut builder = Entity::builder();

        builder.tag(widget());
        if let Some(parent) = parent {
            builder.tag(child_of(parent));
        }

        let mut world = app.world();
        let id = builder.spawn(&mut world);

        drop(world);
        Fragment { id, app }
    }

    /// Acquire a lock to the world to modify the fragment
    pub fn write(&mut self) -> FragmentRef {
        FragmentRef {
            world: self.app.world(),
            fragment: self,
        }
    }
    /// Render a widget in this fragment.
    ///
    /// This is used to yield a whole widget to the fragment
    pub async fn put<W: Widget>(&mut self, widget: W) -> W::Output {
        self.write().clear();
        widget.render(self).await
    }

    /// Returns a handle used to control the app
    pub fn app(&self) -> &AppRef {
        &self.app
    }

    /// Attach another fragment as a child
    pub fn attach<'a, W>(&mut self, widget: W) -> impl Future<Output = W::Output> + 'a
    where
        W: 'a + Widget,
    {
        let app = self.app.clone();
        let id = self.id;
        async move {
            let mut child = Fragment::spawn(app, Some(id));
            widget.render(&mut child).await
        }
    }
}

pub struct FragmentRef<'a> {
    world: MutexGuard<'a, World>,
    fragment: &'a Fragment,
}

impl<'a> FragmentRef<'a> {
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Sets a component value
    pub fn set<T: ComponentValue>(&mut self, component: Component<T>, value: T) -> &mut Self {
        self.world.set(self.fragment.id, component, value).unwrap();
        self
    }

    fn clear(&mut self) -> &mut Self {
        self.world.despawn_children(self.fragment.id, child_of);
        self.world
            .entity_mut(self.fragment.id)
            .unwrap()
            .retain(|k| k == widget().key());

        self
    }
}
