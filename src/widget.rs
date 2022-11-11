use std::sync::MutexGuard;

use async_trait::async_trait;
use flax::{child_of, Component, ComponentValue, Entity, World};
use futures::{
    future::{BoxFuture, LocalBoxFuture},
    Future, FutureExt,
};

use crate::{app::AppRef, components::widget};

#[async_trait]
/// Represents a widget which can be rendered into a fragment of the UI tree.
///
/// Widgets can optionally return a value, which can be used for Input fields or alike.
pub trait Widget: Send {
    type Output;
    async fn render(self, fragment: &mut Fragment) -> Self::Output;
    async fn render_boxed(self: Box<Self>, fragment: &mut Fragment) -> Self::Output {
        self.render(fragment).await
    }
}

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
        self.write().clear();
        widget.render(self).await
    }

    /// Returns a handle used to control the app
    pub fn app(&self) -> &AppRef {
        &self.app
    }

    /// Attach another fragment as a child
    pub fn attach<'w, W>(&mut self, widget: W) -> WidgetFuture<'w, W::Output>
    where
        W: 'w + Widget,
    {
        let mut guard = self.write();
        guard.attach(widget)
    }

    pub fn id(&self) -> Entity {
        self.id
    }
}

pub struct WidgetFuture<'a, T = ()> {
    fut: BoxFuture<'a, T>,
    id: Entity,
}

impl<'a, T> Future for WidgetFuture<'a, T> {
    type Output = T;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.fut.poll_unpin(cx)
    }
}

impl<'a, T> WidgetFuture<'a, T> {
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

    fn clear(&mut self) -> &mut Self {
        self.world.despawn_children(self.fragment.id, child_of);
        self.world
            .entity_mut(self.fragment.id)
            .unwrap()
            .retain(|k| k == widget().key());

        self
    }

    /// Attach another fragment as a child
    pub fn attach<'w, W>(&mut self, widget: W) -> WidgetFuture<'w, W::Output>
    where
        W: 'w + Widget,
    {
        let app = self.fragment.app.clone();
        let id = self.fragment.id;
        let mut child = Fragment::spawn(&mut self.world, app, Some(id));

        WidgetFuture {
            id: child.id,
            fut: Box::pin(async move { widget.render(&mut child).await }),
        }
    }
}

#[async_trait]
impl<W> Widget for Box<W>
where
    W: ?Sized + Widget + Send,
{
    type Output = W::Output;
    async fn render(self, frag: &mut Fragment) -> Self::Output {
        W::render_boxed(self, frag).await
    }
}

/// Helper trait for turning a list of widgets into a list of render futures.
pub trait WidgetCollection {
    /// Convert the collection into fragments
    fn attach(self, parent: &mut Fragment) -> Vec<WidgetFuture<'static>>;
}

impl WidgetCollection for Vec<Box<dyn Widget<Output = ()> + Send>> {
    fn attach(self, parent: &mut Fragment) -> Vec<WidgetFuture<'static>> {
        let mut guard = parent.write();
        self.into_iter().map(|w| guard.attach(w)).collect()
    }
}

macro_rules! tuple_impl {
    ($($idx: tt => $ty: ident),*) => {
        impl<$($ty: Widget<Output = ()> + 'static + Send,)*> WidgetCollection for ($($ty,)*) {
            fn attach(self, parent: &mut Fragment) -> Vec<WidgetFuture<'static>> {
                let mut guard = parent.write();
                vec![$( guard.attach(self.$idx),)*]
            }
        }
    };
}

tuple_impl! { 0 => A }
tuple_impl! { 0 => A, 1 => B }
tuple_impl! { 0 => A, 1 => B, 2 => C }
tuple_impl! { 0 => A, 1 => B, 2 => C, 3 => D }
