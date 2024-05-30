use async_scoped;
use blight;
use dirs;
use eframe::egui;
use egui::ViewportBuilder;
use fs2::FileExt;
use futures_signals::signal::{self, SignalExt};
use std::fs::File;
use std::io::Result;
use std::path::PathBuf;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

const PROG_NAME: &str = "brightness-slider";

#[tokio::main]
async fn main() -> Result<()> {
    let runtime_dir = dirs::runtime_dir()
        .expect("Failed to get runtime dir")
        .join(PROG_NAME);

    std::fs::create_dir_all(&runtime_dir)?;

    let socket_path = runtime_dir.join("brightness-slider.sock");
    let lock_path = runtime_dir.join("brightness-slider.lock");
    let lock_file = File::create(lock_path)?;

    match lock_file.try_lock_exclusive() {
        Ok(_) => {
            println!("Lock acquired");
            brightness_slider(socket_path).await;
        }
        Err(_) => {
            println!("Another instance is already running.");
            println!("Starting client");
            brightness_slider_client(&socket_path).await?;
        }
    }

    Ok(())
}

async fn brightness_slider_client(socket_path: &PathBuf) -> Result<()> {
    let mut client = UnixStream::connect(socket_path).await?;

    client.write_all(b"ping").await?;
    println!("Sent ping to server. Exiting.");

    Ok(())
}

fn write_brightness_to_device(
    device: &mut blight::Device,
    target_brightness: u8,
) -> blight::BlResult<()> {
    let max: u32 = device.max();
    let value = (max as f64 * target_brightness as f64 / 100.0) as u32;

    device.write_value(value)
}

async fn device_write_thread(
    rx: signal::MutableSignal<u8>,
    mut device: blight::Device,
) -> Result<()> {
    rx.for_each(|target_brightness| {
        write_brightness_to_device(&mut device, target_brightness)
            .expect("Failed to write brightness");
        async {}
    })
    .await;

    // gui has closed, exit
    Ok(())
}

async fn server_thread(socket_path: &PathBuf, brightness: signal::Mutable<u8>) -> Result<()> {
    println!("Starting server thread");

    // Ensure no stale socket file
    match std::fs::remove_file(socket_path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let listener = UnixListener::bind(socket_path)?;
    println!("Listening on socket: {}", socket_path.display());
    loop {
        let (mut stream, _addr) = listener.accept().await?;
        let mut recv_buf = String::new();
        stream.read_to_string(&mut recv_buf).await?;
        println!("Received command from client: {}", recv_buf);
        match recv_buf.as_str() {
            "ping" => {
                brightness.set(100);
            }
            _ => {}
        }
    }
}

async fn brightness_slider(socket_path: PathBuf) {
    let ((), _outputs) = async_scoped::TokioScope::scope_and_block(|s| {
        let device = blight::Device::new(None).expect("Failed to get device");
        let curr_brightness: u8 = device.current_percent().round() as u8;
        println!("Initial brightness: {}", curr_brightness);

        let brightness = signal::Mutable::new(curr_brightness);

        s.spawn_cancellable(device_write_thread(brightness.signal(), device), || Ok(()));
        s.spawn_cancellable(server_thread(&socket_path, brightness.clone()), || Ok(()));

        run_gui(brightness.clone());

        s.cancel();
    });
}

fn run_gui(brightness: signal::Mutable<u8>) {
    let app = BrightnessApp::new(brightness);
    let window_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([15.0, 130.0]),
        window_builder: Some(Box::new(|builder: ViewportBuilder| {
            builder
                .with_window_type(egui::X11WindowType::Dialog)
                .with_decorations(false)
        })),
        ..Default::default()
    };
    eframe::run_native("Brightness", window_options, Box::new(|_cc| Box::new(app))).unwrap();
}

#[derive(Debug)]
struct BrightnessApp {
    // percentage
    brightness: signal::Mutable<u8>,
}

impl BrightnessApp {
    fn new(brightness: signal::Mutable<u8>) -> Self {
        Self { brightness }
    }

    fn add_target_brightness(&mut self, delta: i16) {
        self.brightness
            .replace_with(|b| (*b as i16 + delta).clamp(0, 100) as u8);
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // can't use ctx inside input closure
        let quit = ctx.input(|i| i.key_pressed(egui::Key::Q) || i.key_pressed(egui::Key::Escape));
        if quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        ctx.input(|i| {
            // arrow key control
            if i.key_pressed(egui::Key::ArrowUp) {
                self.add_target_brightness(5);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.add_target_brightness(-5);
            }

            // pgup pgdown control
            if i.key_pressed(egui::Key::PageUp) {
                self.add_target_brightness(20);
            }
            if i.key_pressed(egui::Key::PageDown) {
                self.add_target_brightness(-20);
            }

            // mouse wheel control
            if i.raw_scroll_delta.y > 0.0 {
                self.add_target_brightness(5);
            } else if i.raw_scroll_delta.y < 0.0 {
                self.add_target_brightness(-5);
            }
        });
    }
}

impl eframe::App for BrightnessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let slider = egui::Slider::from_get_set(0.0..=100.0, |v| {
                if let Some(v) = v {
                    self.brightness.set(v.round() as u8);
                }
                self.brightness.get() as f64
            })
            .show_value(false)
            .vertical();

            ui.add(slider);

            self.handle_input(ctx);
        });
    }
}
