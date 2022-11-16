use std::sync::MutexGuard;

use async_trait::async_trait;
use flax::{child_of, Component, ComponentValue, Entity, World};
use futures::{future::BoxFuture, Future, FutureExt};

use crate::{app::AppRef, components::widget, fragment::Fragment};

/// Represents a widget which can be rendered into a fragment of the UI tree.
///
/// Widgets can optionally return a value, which can be used for Input fields or alike.
#[async_trait]
pub trait Widget: Send {
    type Output;
    /// Mounts the widget, returning a future which updates and keeps track of the state.
    async fn mount(self, fragment: Fragment) -> Self::Output;
}

#[async_trait]
pub(crate) trait BoxedWidget: Send {
    type Output;
    async fn mount_boxed(self: Box<Self>, fragment: Fragment) -> Self::Output;
}

#[async_trait]
impl<W> BoxedWidget for W
where
    W: ?Sized + Widget,
{
    type Output = W::Output;

    async fn mount_boxed(self: Box<Self>, fragment: Fragment) -> W::Output {
        (self).mount(fragment).await
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
    pub(crate) fn new(id: Entity, fut: BoxFuture<'a, T>) -> Self {
        Self { fut, id }
    }

    pub fn id(&self) -> Entity {
        self.id
    }
}

#[async_trait]
impl<W> Widget for Box<W>
where
    W: ?Sized + Widget,
{
    type Output = W::Output;

    async fn mount(self, frag: Fragment) -> Self::Output {
        self.mount_boxed(frag).await
    }
}

/// Helper trait for turning a list of widgets into a list of render futures.
pub trait WidgetCollection {
    /// Convert the collection into fragments
    fn attach(self, parent: &mut Fragment) -> Vec<WidgetFuture<'static>>;
}

impl WidgetCollection for Vec<Box<dyn Widget<Output = ()> + Send>> {
    fn attach(self, parent: &mut Fragment) -> Vec<WidgetFuture<'static>> {
        self.into_iter().map(|w| parent.attach_boxed(w)).collect()
    }
}

macro_rules! tuple_impl {
    ($($idx: tt => $ty: ident),*) => {
        impl<$($ty: Widget<Output = ()> + 'static + Send,)*> WidgetCollection for ($($ty,)*) {
            fn attach(self, parent: &mut Fragment) -> Vec<WidgetFuture<'static>> {
                vec![$( parent.attach(self.$idx),)*]
            }
        }
    };
}

tuple_impl! { 0 => A }
tuple_impl! { 0 => A, 1 => B }
tuple_impl! { 0 => A, 1 => B, 2 => C }
tuple_impl! { 0 => A, 1 => B, 2 => C, 3 => D }

