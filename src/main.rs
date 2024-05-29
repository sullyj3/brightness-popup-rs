use eframe::egui;
use egui::ViewportBuilder;

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

    let initial_state: Box<BrightnessApp> = Box::new(BrightnessApp { brightness: 100 });

    eframe::run_native("Brightness", options, Box::new(|_cc| initial_state))
}

#[derive(Debug, Default)]
struct BrightnessApp {
    brightness: i32,
}

// TODO: depend on blight
impl BrightnessApp {
    fn set_brightness(&mut self, brightness: i32) {
        let brightness = brightness.clamp(0, 100);
        self.brightness = brightness;
    }
    
    fn increase_brightness(&mut self, delta: i32) {
        self.set_brightness(self.brightness + delta);
    }

    fn decrease_brightness(&mut self, delta: i32) {
        self.set_brightness(self.brightness - delta);
    }
}

impl eframe::App for BrightnessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(
                egui::Slider::new(&mut self.brightness, 0..=100)
                    .show_value(false)
                    .vertical(),
            );
            println!("Brightness: {}", self.brightness);

            let quit =
                ctx.input(|i| i.key_pressed(egui::Key::Q) || i.key_pressed(egui::Key::Escape));
            if quit {
                std::process::exit(0)
            }

            // arrow key control
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                self.increase_brightness(5);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                self.decrease_brightness(5);
            }

            // pgup pgdown control
            if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
                self.increase_brightness(20);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
                self.decrease_brightness(20);
            }
        });
    }
}
