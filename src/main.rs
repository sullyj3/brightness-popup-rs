use eframe::egui;
use egui::ViewportBuilder;
use blight::{self, BlResult};

fn main() -> Result<(), eframe::Error> {
    let window_builder_hook = Box::new(|builder: ViewportBuilder| {
        builder
            .with_window_type(egui::X11WindowType::Dialog)
            // .with_resizable(false)
            .with_decorations(false)
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([15.0, 130.0]),
        window_builder: Some(window_builder_hook),
        ..Default::default()
    };

    let Ok(device) = blight::Device::new(None) else {
        panic!("Failed to get backlight device");
    };

    let current_brightness = device.current_percent().round();
    println!("Initial brightness: {}", current_brightness);

    let initial_state: Box<BrightnessApp> = Box::new(BrightnessApp::new(device));

    eframe::run_native("Brightness", options, Box::new(|_cc| initial_state))
}

#[derive(Debug)]
struct BrightnessApp {
    device: blight::Device,
    // percentage
    target_brightness: u8,
}

impl BrightnessApp {
    fn new(device: blight::Device) -> Self {
        let target_brightness = device.current_percent() as u8;
        Self {
            device,
            target_brightness,
        }
    }

    // we use this stateful style because the egui slider takes a mutable reference
    // to control, as opposed to providing a callback or event handler
    fn write_brightness_to_device(&mut self) -> BlResult<()> {
        let max: u32 = self.device.max();
        let value = (max as f64 * self.target_brightness as f64 / 100.0) as u32;

        self.device.write_value(value)
    }

    fn add_target_brightness(&mut self, delta: i16) {
        self.target_brightness = (self.target_brightness as i16 + delta)
            .clamp(0, 100) as u8;
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let quit =
            ctx.input(|i| i.key_pressed(egui::Key::Q) || i.key_pressed(egui::Key::Escape));
        if quit {
            self.device.reload();
            println!("Final brightness: {}", self.device.current_percent().round());
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
