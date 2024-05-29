use async_scoped;
use blight;
use eframe::egui;
use egui::ViewportBuilder;
use fs2::FileExt;
use std::fs::File;
use std::io::Result;
use std::path::PathBuf;
use std::sync::mpsc;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use dirs;

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

async fn device_write_thread(rx: mpsc::Receiver<u8>, mut device: blight::Device) -> Result<()> {
    loop {
        let Ok(target_brightness) = rx.recv() else {
            break;
        };
        write_brightness_to_device(&mut device, target_brightness)
            .expect("Failed to write brightness");
    }

    // gui has closed, report final brightness and exit
    device.reload();
    Ok(())
}

async fn server_thread(socket_path: &PathBuf) -> Result<()> {
    println!("Starting server thread");

    // Ensure no stale socket file
    match std::fs::remove_file(socket_path) {
        Ok(_) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(e),
    }

    let listener = UnixListener::bind(socket_path)?;
    println!("Listening on socket: {}", socket_path.display());
    loop {
        let (mut stream, _addr) = listener.accept().await?;
        let mut recv_buf = String::new();
        stream.read_to_string(&mut recv_buf).await?;
        println!("Received command from client: {}", recv_buf);
    }
}

async fn brightness_slider(socket_path: PathBuf) {
    let ((), _outputs) = async_scoped::TokioScope::scope_and_block(|s| {
        let window_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([15.0, 130.0]),
            window_builder: Some(Box::new(|builder: ViewportBuilder| {
                builder
                    .with_window_type(egui::X11WindowType::Dialog)
                    .with_decorations(false)
            })),
            ..Default::default()
        };

        let device = blight::Device::new(None).expect("Failed to get device");
        let curr_brightness: u8 = device.current_percent().round() as u8;
        println!("Initial brightness: {}", curr_brightness);

        // TODO: consider using tokio::sync::watch::channel.
        // This drops all values except the most recent one, which is what we want.
        // https://docs.rs/tokio/latest/tokio/sync/watch/fn.channel.html
        // another possibility is `Eventual`
        // https://docs.rs/crate/eventuals/0.6.7
        //
        // comparison of watch and eventual from the author of eventual:
        // https://www.reddit.com/r/rust/comments/oo917a/comment/h6ygv60/
        //
        // another comment mentions that eventuals are like FRP signals
        // see also https://lib.rs/crates/futures-signals (more stars + more recent commits)
        let (tx_brightness, rx_brightness) = mpsc::channel();

        // TODO ensure these are cancelled when the egui window is closed
        s.spawn(device_write_thread(rx_brightness, device));
        s.spawn(server_thread(&socket_path));

        let app = BrightnessApp::new(tx_brightness, curr_brightness);
        eframe::run_native("Brightness", window_options, Box::new(|_cc| Box::new(app))).unwrap();
    });
}

#[derive(Debug)]
struct BrightnessApp {
    // percentage
    target_brightness: u8,
    tx_brightness: mpsc::Sender<u8>,
}

impl BrightnessApp {
    fn new(tx_brightness: mpsc::Sender<u8>, curr_brightness: u8) -> Self {
        Self {
            target_brightness: curr_brightness,
            tx_brightness,
        }
    }

    fn add_target_brightness(&mut self, delta: i16) {
        self.target_brightness = (self.target_brightness as i16 + delta).clamp(0, 100) as u8;
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let quit = ctx.input(|i| i.key_pressed(egui::Key::Q) || i.key_pressed(egui::Key::Escape));
        if quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // arrow key control
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.add_target_brightness(5);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            self.add_target_brightness(-5);
        }

        // pgup pgdown control
        if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
            self.add_target_brightness(20);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
            self.add_target_brightness(-20);
        }
    }
}

impl eframe::App for BrightnessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(
                // todo: try Slider::from_get_set
                egui::Slider::new(&mut self.target_brightness, 0..=100)
                    .show_value(false)
                    .vertical(),
            );

            self.handle_input(ctx);
            self.tx_brightness
                .send(self.target_brightness)
                .expect("Bg thread hung up unexpectedly");
        });
    }
}
