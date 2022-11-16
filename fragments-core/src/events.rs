use flax::{entity_ids, Component, ComponentValue, Entity, Query, World};
use futures_signals::signal::Mutable;

// pub trait EventHandler<T>: ComponentValue {
//     fn on_event(&mut self, id: Entity, world: &World, event: &T);
// }

// impl<F, T> EventHandler<T> for F
// where
//     F: FnMut(Entity, &World, &T) + ComponentValue,
// {
//     fn on_event(&mut self, id: Entity, world: &World, event: &T) {
//         (self)(id, world, event)
//     }
// }

// impl<T> EventHandler<T> for flume::Sender<T>
// where
//     T: 'static + Send + Clone,
// {
//     fn on_event(&mut self, _: Entity, _: &World, event: &T) {
//         self.send(event.clone()).ok();
//     }
// }

// impl<T> EventHandler<T> for Mutable<T>
// where
//     T: 'static + Send + Sync + Clone,
// {
//     fn on_event(&mut self, _: Entity, _: &World, event: &T) {
//         self.set(event.clone())
//     }
// }

// impl<T> EventHandler<T> for futures_signals::signal::Sender<T>
// where
//     T: 'static + Send + Clone,
// {
//     fn on_event(&mut self, _: Entity, _: &World, event: &T) {
//         self.send(event.clone()).ok();
//     }
// }

pub type EventHook<T> = Box<dyn FnMut(Entity, &World, &T) + Send + Sync>;

/// Send an event to all hooks in the world
pub fn send_event<T: Sync>(world: &World, event: Component<EventHook<T>>, event_data: T)
where
    EventHook<T>: 'static,
{
    Query::new((entity_ids(), event.as_mut()))
        .borrow(world)
        .iter()
        .for_each(|(id, handler)| handler(id, world, &event_data))
}
