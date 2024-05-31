use async_scoped;
use blight;
use dirs;
use fs2::FileExt;
use futures_signals::signal::{self, SignalExt};
use std::fs::File;
use std::io::Result;
use std::path::PathBuf;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

mod gui;
use crate::gui::run_gui;

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

fn usage() {
    println!("Usage: {} [inc|dec|set] <value>", PROG_NAME);
    std::process::exit(1);
}

fn cli_invalid_args() -> ! {
    println!("gui is already running, and no valid command was given.");
    println!("Exiting...");
    std::process::exit(1);
}

async fn brightness_slider_client(socket_path: &PathBuf) -> Result<()> {
    let mut client = UnixStream::connect(socket_path).await?;

    // get args, skipping the first arg which is the program name
    let args: Vec<String> = std::env::args().skip(1).collect();

    // parse args
    if args.len() <= 1 {
        cli_invalid_args();
    } else if args.len() == 2 {
        match args[0].as_str() {
            "inc" => {
                // ensure the second arg is a number
                let Ok(_) = args[1].parse::<u8>() else {
                    usage();
                    std::process::exit(1);
                };
                client
                    .write_all(format!("inc {}", args[1]).as_bytes())
                    .await?;
            }
            "dec" => {
                // ensure the second arg is a number
                let Ok(_) = args[1].parse::<u8>() else {
                    usage();
                    std::process::exit(1);
                };
                client
                    .write_all(format!("dec {}", args[1]).as_bytes())
                    .await?;
            }
            "set" => {
                // ensure the second arg is a number
                let Ok(_) = args[1].parse::<u8>() else {
                    usage();
                    std::process::exit(1);
                };
                client
                    .write_all(format!("set {}", args[1]).as_bytes())
                    .await?;
            }
            _ => cli_invalid_args(),
        }
        Ok(())
    } else {
        cli_invalid_args();
    }
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
