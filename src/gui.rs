use eframe::egui;
use egui::ViewportBuilder;
use futures_signals::signal;

pub fn run_gui(brightness: signal::Mutable<u8>) {
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

fn add_brightness(brightness: u8, delta: i16) -> u8 {
    (brightness as i16 + delta).clamp(0, 100) as u8
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

    fn handle_input(&mut self, ctx: &egui::Context) {
        // can't use ctx inside input closure
        let quit = ctx.input(|i| i.key_pressed(egui::Key::Q) || i.key_pressed(egui::Key::Escape));
        if quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        ctx.input(|i| {
            // arrow key control
            if i.key_pressed(egui::Key::ArrowUp) {
                self.brightness.replace_with(|&mut b| add_brightness(b, 5));
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.brightness.replace_with(|&mut b| add_brightness(b, -5));
            }

            // pgup pgdown control
            if i.key_pressed(egui::Key::PageUp) {
                self.brightness.replace_with(|&mut b| add_brightness(b, 20));
            }
            if i.key_pressed(egui::Key::PageDown) {
                self.brightness
                    .replace_with(|&mut b| add_brightness(b, -20));
            }

            // mouse wheel control
            if i.raw_scroll_delta.y > 0.0 {
                self.brightness.replace_with(|&mut b| add_brightness(b, 5));
            } else if i.raw_scroll_delta.y < 0.0 {
                self.brightness.replace_with(|&mut b| add_brightness(b, -5));
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
