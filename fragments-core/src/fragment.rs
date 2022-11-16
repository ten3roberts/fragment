use std::sync::MutexGuard;

use flax::{child_of, Component, ComponentValue, Entity, World};

use crate::{
    app::AppRef, components::widget, events::EventHook, BoxedWidget, Widget, WidgetFuture,
};

/// Represents a piece of the UI
pub struct Fragment {
    id: Entity,
    app: AppRef,
}

impl Fragment {
    pub(crate) fn spawn(world: &mut World, app: AppRef, parent: Option<Entity>) -> Fragment {
        let mut builder = Entity::builder();

        builder.tag(widget());
        if let Some(parent) = parent {
            builder.tag(child_of(parent));
        }

        let id = builder.spawn(world);

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
        widget
            .mount(Self {
                id: self.id,
                app: self.app().clone(),
            })
            .await
    }

    // Returns a handle used to control the app
    pub fn app(&self) -> &AppRef {
        &self.app
    }

    /// Attach another fragment as a child
    pub fn attach<'w, W>(&mut self, widget: W) -> WidgetFuture<'w, W::Output>
    where
        W: 'w + Widget,
    {
        let app = self.app.clone();
        let id = self.id;
        let child = Fragment::spawn(&mut self.app.world(), app, Some(id));

        WidgetFuture::new(child.id, widget.mount(child))
    }

    /// Attach another fragment as a child
    pub fn attach_boxed<'w, W>(&mut self, widget: Box<W>) -> WidgetFuture<'w, W::Output>
    where
        W: 'w + Widget + ?Sized,
    {
        let app = self.app.clone();
        let id = self.id;
        let child = Fragment::spawn(&mut self.app.world(), app, Some(id));

        WidgetFuture::new(child.id, widget.mount_boxed(child))
    }

    pub fn id(&self) -> Entity {
        self.id
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

    pub fn on_event<T: ComponentValue, F: 'static + FnMut(Entity, &World, &T) + Send + Sync>(
        &mut self,
        event: Component<EventHook<T>>,
        mut handler: F,
    ) -> &mut Self {
        self.set(event, Box::new(handler))
    }

    fn clear(&mut self) -> &mut Self {
        self.world.despawn_children(self.fragment.id, child_of).ok();
        self.world
            .entity_mut(self.fragment.id)
            .unwrap()
            .retain(|k| k == widget().key());

        self
    }
}
