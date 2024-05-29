use async_scoped;
use blight;
use eframe::egui;
use egui::ViewportBuilder;
use std::sync::mpsc;
use tokio;

#[tokio::main]
async fn main() {
    brightness_slider().await;
}

fn write_brightness_to_device(
    device: &mut blight::Device,
    target_brightness: u8,
) -> blight::BlResult<()> {
    let max: u32 = device.max();
    let value = (max as f64 * target_brightness as f64 / 100.0) as u32;

    device.write_value(value)
}

async fn bg_thread(rx: mpsc::Receiver<u8>, mut device: blight::Device) {
    loop {
        let Ok(target_brightness) = rx.recv() else {
            break;
        };
        println!("Target brightness is {}", target_brightness);
        write_brightness_to_device(&mut device, target_brightness)
            .expect("Failed to write brightness");
    }

    // gui has closed, report final brightness and exit
    device.reload();
    println!("Final brightness: {}", device.current_percent().round());
}

async fn brightness_slider() {
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
        s.spawn(bg_thread(rx_brightness, device));

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
