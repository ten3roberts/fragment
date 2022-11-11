use std::{
    io::{stdout, Write},
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    QueueableCommand,
};
use flax::{component, events::ChangeSubscriber, name, Query};
use fragment::{
    app::{App, Event},
    Fragment, Widget,
};
use futures::StreamExt;
use glam::{vec2, Vec2};
use tokio::sync::Notify;

slotmap::new_key_type! { pub struct WidgetKey; }

component! {
    widget: (),
    pos: Vec2,
    content: String,

}

pub struct Text(String);

#[async_trait]
impl Widget for Text {
    type Output = ();
    async fn render(self, fragment: &mut Fragment) {
        fragment
            .write()
            .set(content(), self.0)
            .set(pos(), vec2(0.0, 0.0))
            .set(widget(), ());
    }
}

pub struct Application {}

#[async_trait]
impl Widget for Application {
    type Output = ();
    async fn render(self, fragment: &mut Fragment) {
        fragment
            .write()
            .set(name(), "Application".into())
            .set(content(), "Hello, World!".into())
            .set(pos(), vec2(0.0, 0.0))
            .set(widget(), ());

        tokio::spawn(fragment.attach(Renderer));
        tokio::spawn(fragment.attach(EventHandler));

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let clock = fragment.attach(Clock {
            interval: Duration::from_millis(500),
        });

        clock.await
    }
}

struct Clock {
    interval: Duration,
}

#[async_trait]
impl Widget for Clock {
    type Output = ();
    async fn render(self, frag: &mut Fragment) {
        let start = Instant::now();

        loop {
            let elapsed = start.elapsed();
            frag.put(Text(format!("Elapsed: {:?}", elapsed))).await;

            tokio::time::sleep(self.interval).await
        }
    }
}

struct EventHandler;
#[async_trait]
impl Widget for EventHandler {
    type Output = eyre::Result<()>;
    async fn render(self, state: &mut Fragment) -> eyre::Result<()> {
        let mut events = crossterm::event::EventStream::new();

        state.write().set(pos(), vec2(10.0, 10.0)).set(widget(), ());

        let app = state.app().clone();

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
    }
}

struct Renderer;

#[async_trait]
impl Widget for Renderer {
    type Output = eyre::Result<()>;
    async fn render(self, state: &mut Fragment) -> eyre::Result<()> {
        let mut stdout = stdout();

        let ui_changed = Arc::new(Notify::new());
        state.app().world().subscribe(ChangeSubscriber::new(
            &[pos().key(), content().key()],
            Arc::downgrade(&ui_changed),
        ));

        let mut draw_query = Query::new((pos(), content())).with(widget());

        enable_raw_mode().unwrap();

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
