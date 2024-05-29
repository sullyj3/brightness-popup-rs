use async_scoped;
use blight::{self, BlResult};
use eframe::egui;
use egui::ViewportBuilder;
use tokio;

#[tokio::main]
async fn main() {
    brightness_slider().await;
}

async fn background_thread() {
    println!("Hello from the background thread!");
}

async fn brightness_slider() {
    let app = BrightnessApp::default();

    let current_brightness = app.device.current_percent().round();
    println!("Initial brightness: {}", current_brightness);

    let ((), _outputs) = async_scoped::TokioScope::scope_and_block(|s| {
        let window_builder_hook = Box::new(|builder: ViewportBuilder| {
            builder
                .with_window_type(egui::X11WindowType::Dialog)
                .with_decorations(false)
        });

        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([15.0, 130.0]),
            window_builder: Some(window_builder_hook),
            ..Default::default()
        };

        s.spawn(background_thread());
        eframe::run_native("Brightness", options, Box::new(|_cc| Box::new(app))).unwrap();
    });
}

#[derive(Debug)]
struct BrightnessApp {
    device: blight::Device,
    // percentage
    target_brightness: u8,
}

impl BrightnessApp {
    // we use this stateful style because the egui slider takes a mutable reference
    // to control, as opposed to providing a callback or event handler
    fn write_brightness_to_device(&mut self) -> BlResult<()> {
        let max: u32 = self.device.max();
        let value = (max as f64 * self.target_brightness as f64 / 100.0) as u32;

        self.device.write_value(value)
    }

    fn add_target_brightness(&mut self, delta: i16) {
        self.target_brightness = (self.target_brightness as i16 + delta).clamp(0, 100) as u8;
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let quit = ctx.input(|i| i.key_pressed(egui::Key::Q) || i.key_pressed(egui::Key::Escape));
        if quit {
            self.device.reload();
            println!(
                "Final brightness: {}",
                self.device.current_percent().round()
            );
            std::process::exit(0)
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

impl Default for BrightnessApp {
    fn default() -> Self {
        let device = blight::Device::new(None).expect("Failed to get backlight device");
        let target_brightness = device.current_percent() as u8;
        Self {
            device,
            target_brightness,
        }
    }
}

impl eframe::App for BrightnessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(
                egui::Slider::new(&mut self.target_brightness, 0..=100)
                    .show_value(false)
                    .vertical(),
            );

            self.handle_input(ctx);
            if let Err(e) = self.write_brightness_to_device() {
                eprintln!("Failed to write brightness: {}", e);
            }
        });
    }
}
