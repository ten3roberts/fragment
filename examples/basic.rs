use std::time::Duration;

use async_trait::async_trait;
use flax::name;
use fragment::{fragment::Fragment, state::State};

pub struct Application {}

#[async_trait]
impl Fragment for Application {
    async fn render(self, state: fragment::fragment::FragmentState) {
        eprintln!("Drawing application");
        state.write().set(name(), "Application".into());

        tokio::time::sleep(Duration::from_millis(1000)).await;

        eprintln!("One second later")
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
