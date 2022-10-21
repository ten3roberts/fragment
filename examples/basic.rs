use std::time::{Duration, Instant};

use async_trait::async_trait;
use flax::{component, name};
use fragment::{
    fragment::{Fragment, FragmentState},
    state::State,
};
use futures::{join, TryFutureExt};

component! {
    text: String,
}

pub struct Application {}

#[async_trait]
impl Fragment for Application {
    async fn render(self, state: fragment::fragment::FragmentState) {
        eprintln!("Drawing application");
        state
            .write()
            .set(name(), "Application".into())
            .set(text(), "Application".into());

        tokio::time::sleep(Duration::from_millis(1000)).await;

        eprintln!("One second later");

        state
            .attach(Clock {
                interval: Duration::from_millis(500),
            })
            .await;
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        eprintln!("Dropping app")
    }
}

struct Clock {
    interval: Duration,
}

struct Renderer {}

#[async_trait]
impl Fragment for Renderer {
    async fn render(self, state: fragment::fragment::FragmentState) {}
}

#[async_trait]
impl Fragment for Clock {
    async fn render(self, state: FragmentState) {
        let start = Instant::now();

        loop {
            let elapsed = start.elapsed();
            eprintln!("Elapsed: {:?}", elapsed);
            state.write().set(text(), format!("Elapsed: {:?}", elapsed));
            tokio::time::sleep(self.interval).await
        }
    }
}

fn main() {
    let mut state = State::new();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let root = state.spawn_fragment(Application {});
            // Execute the root concurrently
            tokio::spawn(root);

            state.run().await.unwrap()
        })
}
