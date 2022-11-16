use std::sync::Arc;

use async_trait::async_trait;
use fragments_core::{
    app::{self, App},
    components,
    events::{send_event, EventHook},
    Widget,
};
use futures::future::BoxFuture;
use futures_signals::signal::{Mutable, SignalExt};
use tracing_subscriber::{prelude::*, Registry};
use tracing_tree::HierarchicalLayer;
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyboardInput, WindowEvent},
    event_loop::{EventLoop, EventLoopBuilder},
    window::{Window, WindowBuilder, WindowId},
};

struct GraphicsState {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

impl GraphicsState {
    // Creates a new graphics state
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        tracing::info!("Creating instance");
        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        tracing::info!("Found adapter: {adapter:?}");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        tracing::info!("Found device: {device:?}");

        // let modes = surface.get_supported_modes(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };

        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
            size,
        }
    }

    fn on_resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn on_event(&mut self, event: &WindowEvent) -> bool {
        todo!()
    }

    fn update(&mut self) {
        todo!()
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        todo!()
    }
}

pub struct GraphicsLayer {
    window: Arc<Window>,
}

#[async_trait]
impl Widget for GraphicsLayer {
    type Output = eyre::Result<()>;

    async fn mount(self, mut fragment: fragments_core::Fragment) -> Self::Output {
        let Self { window } = self;
        let state = Mutable::new(GraphicsState::new(&window).await);

        fragment
            .write()
            .on_event(on_resize(), move |_, _, new_size: &PhysicalSize<u32>| {
                tracing::info!("Resizing: {new_size:?}");
                state.lock_mut().on_resize(*new_size);
            })
            .on_event(on_keyboard_input(), move |_, _, input| {
                tracing::info!(?input, "Input");
            })
            .on_event(on_char_typed(), move |_, _, c| {
                tracing::info!(?c, "Character");
            });

        Ok(())
    }
}

struct WindowLayer {
    title: String,
}

flax::component! {
    on_keyboard_input: EventHook<KeyboardInput>,
    on_char_typed: EventHook<char>,
    on_window_close: EventHook<WindowId>,
    on_resize: EventHook<PhysicalSize<u32>>,

    graphics_state: GraphicsState,

    resources,
}

#[async_trait]
impl Widget for WindowLayer {
    type Output = eyre::Result<()>;

    async fn mount(self, mut fragment: fragments_core::Fragment) -> Self::Output {
        let events = EventLoop::new();
        let window = Arc::new(WindowBuilder::new().with_title(self.title).build(&events)?);
        let app = fragment.app().clone();
        tokio::spawn(fragment.attach(GraphicsLayer {
            window: window.clone(),
        }));

        events.run(move |event, _, ctl| {
            let _window = &window;

            match event {
                Event::WindowEvent { window_id, event } => match event {
                    winit::event::WindowEvent::CloseRequested => {
                        app.enqueue(app::Event::Exit).ok();
                        ctl.set_exit();
                    }
                    WindowEvent::Resized(new_size) => {
                        send_event(&app.world(), on_resize(), new_size)
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        send_event(&app.world(), on_keyboard_input(), input)
                    }
                    WindowEvent::ReceivedCharacter(c) => {
                        send_event(&app.world(), on_char_typed(), c)
                    }
                    _ => {}
                },
                _ => {}
            }
        });
    }
}

fn application() -> impl Widget {
    WindowLayer {
        title: "Fragments".into(),
    }
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let subscriber = Registry::default().with(HierarchicalLayer::new(2));
    tracing::subscriber::set_global_default(subscriber).unwrap();
    tracing::info!("Starting");

    App::new().run(application()).await;
    Ok(())
}
