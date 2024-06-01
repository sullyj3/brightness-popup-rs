use async_scoped;
use blight;
use dirs;
use fs2::FileExt;
use futures_signals::signal::{self, SignalExt};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

mod gui;
mod brightness;

use crate::gui::run_gui;
use brightness::add_brightness;


const PROG_NAME: &str = "brightness-slider";

#[tokio::main]
async fn main() -> io::Result<()> {
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

fn cli_invalid_args() -> ! {
    println!("gui is already running, and no valid command was given.");
    println!("Exiting...");
    std::process::exit(1);
}

#[derive(Debug, Serialize, Deserialize)]
enum Command {
    Inc(u8),
    Dec(u8),
    Set(u8),
}

fn parse_command_from_args(command: &[String]) -> Option<Command> {
    if command.len() != 2 {
        return None;
    }
    let value = command[1].parse::<u8>().ok()?;
    match command[0].as_str() {
        "inc" => Some(Command::Inc(value)),
        "dec" => Some(Command::Dec(value)),
        "set" => Some(Command::Set(value)),
        _ => None,
    }
}

async fn brightness_slider_client(socket_path: &PathBuf) -> io::Result<()> {
    let mut client = UnixStream::connect(socket_path).await?;

    // skip the program name
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = parse_command_from_args(&args) else {
        cli_invalid_args();
    };
    let bytes = serde_json::to_vec(&command).expect("Failed to serialize command");
    client.write_all(&bytes).await
}

fn write_brightness_to_device(
    device: &mut blight::Device,
    target_brightness: u8,
) -> blight::BlResult<()> {
    let max: u32 = device.max();
    let value = (max as f64 * target_brightness as f64 / 100.0) as u32;

    device.write_value(value)
}

async fn device_write_thread(rx: signal::MutableSignal<u8>, mut device: blight::Device) {
    rx.for_each(|target_brightness| {
        write_brightness_to_device(&mut device, target_brightness)
            .expect("Failed to write brightness");
        async {}
    })
    .await;
}

async fn server_thread(socket_path: &PathBuf, brightness: signal::Mutable<u8>) -> io::Result<()> {
    println!("Starting server thread");

    // Ensure no stale socket file
    match std::fs::remove_file(socket_path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let listener = UnixListener::bind(socket_path)?;
    println!("Listening on socket: {}", socket_path.display());
    let mut recv_buf = String::new();
    loop {
        let (mut stream, _addr) = listener.accept().await?;
        recv_buf.clear();
        stream.read_to_string(&mut recv_buf).await?;

        let command: Command = match serde_json::from_str(&recv_buf) {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to parse command: {:?}", e);
                continue;
            }
        };

        match command {
            Command::Inc(delta) => {
                brightness.replace_with(|&mut b| add_brightness(b, delta as i16));
            },
            Command::Dec(delta) => {
                brightness.replace_with(|&mut b| add_brightness(b, -(delta as i16)));
            },
            Command::Set(value) => {
                brightness.set(value);
            },
        }
            
    }
}

async fn brightness_slider(socket_path: PathBuf) {
    let ((), _outputs) = async_scoped::TokioScope::scope_and_block(|s| {
        let device = blight::Device::new(None).expect("Failed to get device");
        let curr_brightness: u8 = device.current_percent().round() as u8;
        println!("Initial brightness: {}", curr_brightness);

        // most important part of the program
        let brightness = signal::Mutable::new(curr_brightness);
        let brightness_signal = brightness.signal();

        s.spawn_cancellable(
            async {
                device_write_thread(brightness_signal, device).await;
                Ok(())
            },
            || Ok(()),
        );
        s.spawn_cancellable(server_thread(&socket_path, brightness.clone()), || Ok(()));

        run_gui(brightness.clone());

        s.cancel();
    });
}
