use std::{
    io::{stdout, Write},
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    QueueableCommand,
};
use flax::{
    child_of, component,
    events::{ChangeSubscriber, SubscriberFilterExt},
    name, Query,
};
use fragment::{
    app::{App, Event},
    components::widget,
    Fragment, Widget, WidgetCollection,
};
use futures::{future::BoxFuture, join, stream::FuturesUnordered, StreamExt};
use glam::{vec2, Vec2};
use itertools::Itertools;
use tokio::sync::Notify;

slotmap::new_key_type! { pub struct WidgetKey; }

component! {
    position: Vec2,
    size: Vec2,
    content: String,

}

pub struct Row<W: WidgetCollection> {
    widgets: W,
    padding: f32,
}

impl<W: WidgetCollection> Row<W> {
    pub fn new(widgets: W) -> Self {
        Self {
            widgets,
            padding: 2.0,
        }
    }
}

impl<W: WidgetCollection + Send> Widget for Row<W> {
    type Output = ();
    fn mount(self, mut frag: Fragment) -> BoxFuture<'static, ()> {
        let futures = self.widgets.attach(&mut frag);

        let ids = futures.iter().map(|v| v.id()).collect_vec();
        let mut futures = futures.into_iter().collect::<FuturesUnordered<_>>();

        let width_changed = Arc::new(Notify::new());

        let app = frag.app().clone();

        Box::pin(async move {
            let update_layout = async {
                app.world().subscribe(
                    ChangeSubscriber::new(&[size().key()], Arc::downgrade(&width_changed))
                        .filter(child_of(frag.id()).with()),
                );

                let mut query = Query::new((size(), position().as_mut())).with(child_of(frag.id()));

                loop {
                    width_changed.notified().await;
                    println!("Updating layout for {ids:?}");

                    {
                        let mut guard = frag.write();
                        let mut cursor = Vec2::ZERO;
                        {
                            let world = guard.world();

                            let mut q = query.borrow(world);

                            // Reposition the children
                            for &id in &ids {
                                let (size, pos) = q.get(id).unwrap();
                                *pos = cursor;
                                cursor += *size * Vec2::X + self.padding * Vec2::X;
                            }
                        }
                        // Update the root size
                        guard.set(size(), cursor);
                    }
                }
            };

            let update_loop = async { while let Some(()) = futures.next().await {} };

            join!(update_loop, update_layout);
        })
    }
}

pub struct Text(String);

impl Widget for Text {
    type Output = ();
    fn mount(self, mut fragment: Fragment) -> BoxFuture<'static, ()> {
        fragment
            .write()
            .set(size(), vec2(self.0.len() as f32, 1.0))
            .set(content(), self.0)
            .set(position(), vec2(0.0, 0.0))
            .set(widget(), ());

        Box::pin(async {})
    }
}

pub struct Application {}

impl Widget for Application {
    type Output = ();
    fn mount(self, mut fragment: Fragment) -> BoxFuture<'static, ()> {
        fragment
            .write()
            .set(name(), "Application".into())
            .set(content(), "Hello, World!".into())
            .set(position(), vec2(0.0, 0.0))
            .set(widget(), ());

        tokio::spawn(fragment.attach(Renderer));
        tokio::spawn(fragment.attach(EventHandler));

        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;

            let clock = Clock {
                interval: Duration::from_millis(500),
            };

            let clock2 = Clock {
                interval: Duration::from_millis(1000),
            };

            fragment.put(Row::new((clock, clock2))).await
        })
    }
}

struct Clock {
    interval: Duration,
}

impl Widget for Clock {
    type Output = ();
    fn mount(self, mut frag: Fragment) -> BoxFuture<'static, ()> {
        let start = Instant::now();

        Box::pin(async move {
            loop {
                let elapsed = start.elapsed();
                frag.put(Text(format!("Elapsed: {:?}", elapsed))).await;

                tokio::time::sleep(self.interval).await
            }
        })
    }
}

struct EventHandler;
impl Widget for EventHandler {
    type Output = eyre::Result<()>;
    fn mount(self, mut state: Fragment) -> BoxFuture<'static, eyre::Result<()>> {
        let mut events = crossterm::event::EventStream::new();

        state
            .write()
            .set(position(), vec2(10.0, 10.0))
            .set(widget(), ());

        let app = state.app().clone();

        Box::pin(async move {
            while let Some(Ok(event)) = events.next().await {
                state.write().set(content(), format!("{event:?}"));
                match event {
                    crossterm::event::Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        ..
                    })
                    | crossterm::event::Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }) => {
                        app.enqueue(Event::Exit)?;
                    }
                    _ => {}
                }
            }

            Ok(())
        })
    }
}

struct Renderer;

impl Widget for Renderer {
    type Output = eyre::Result<()>;
    fn mount(self, state: Fragment) -> BoxFuture<'static, eyre::Result<()>> {
        let mut stdout = stdout();

        let ui_changed = Arc::new(Notify::new());
        state.app().world().subscribe(ChangeSubscriber::new(
            &[position().key(), content().key()],
            Arc::downgrade(&ui_changed),
        ));

        let mut draw_query = Query::new((position(), content())).with(widget());

        enable_raw_mode().unwrap();

        Box::pin(async move {
            loop {
                {
                    let world = state.app().world();

                    stdout.queue(Clear(ClearType::All)).unwrap();

                    for (pos, content) in &mut draw_query.borrow(&world) {
                        stdout
                            .queue(cursor::MoveTo(pos.x as _, pos.y as _))
                            .unwrap()
                            .write_all(content.as_bytes())
                            .unwrap();
                    }

                    stdout.flush().unwrap();
                }

                ui_changed.notified().await;
            }
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        disable_raw_mode().unwrap()
    }
}

#[tokio::main]
async fn main() {
    let app = App::new();
    app.run(Application {}).await.unwrap();
}
