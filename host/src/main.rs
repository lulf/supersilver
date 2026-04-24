use std::process::Command;

use futures::StreamExt;
use supersilver_protocol::EncoderState;
use tokio_serial::SerialPortBuilderExt;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let test_mode = args.iter().any(|a| a == "--test");
    let port_name = args.iter().skip(1).find(|a| *a != "--test").cloned().unwrap_or_else(|| {
        eprintln!("Usage: host [--test] <serial-port>");
        eprintln!("  --test  Print volume level instead of calling amixer");
        eprintln!("Available ports:");
        if let Ok(ports) = tokio_serial::available_ports() {
            for p in &ports {
                eprintln!("  {}", p.port_name);
            }
        }
        std::process::exit(1);
    });

    println!("Opening {port_name}");

    let mut port = tokio_serial::new(&port_name, 115200)
        .open_native_async()
        .expect("failed to open serial port");

    #[cfg(unix)]
    port.set_exclusive(false).ok();

    let mut reader = tokio_util::codec::FramedRead::new(port, CobsCodec::new());

    println!("Listening for encoder state...");

    let mut prev_right: Option<i32> = None;
    let mut volume: i32 = 50;

    while let Some(result) = reader.next().await {
        match result {
            Ok(state) => {
                println!("left: {:>4}  right: {:>4}", state.left, state.right);

                if let Some(prev) = prev_right {
                    let delta = state.right - prev;
                    if delta != 0 {
                        if test_mode {
                            volume = (volume + delta).clamp(0, 100);
                            println!("volume: {volume}%");
                        } else {
                            adjust_volume(delta);
                        }
                    }
                }
                prev_right = Some(state.right);
            }
            Err(e) => {
                eprintln!("decode error: {e}");
            }
        }
    }
}

fn adjust_volume(delta: i32) {
    let step = format!("{}%", delta.unsigned_abs());
    let dir = if delta > 0 { "+" } else { "-" };
    let arg = format!("{step}{dir}");

    match Command::new("amixer").args(["sset", "Master", &arg]).output() {
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "amixer failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        Err(e) => eprintln!("failed to run amixer: {e}"),
    }
}

/// Bridges the protocol crate's `Decoder` into a `tokio_util::codec::Decoder`.
struct CobsCodec {
    decoder: supersilver_protocol::Decoder,
}

impl CobsCodec {
    fn new() -> Self {
        Self {
            decoder: supersilver_protocol::Decoder::new(),
        }
    }
}

impl tokio_util::codec::Decoder for CobsCodec {
    type Item = EncoderState;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        let mut result = None;
        let data = src.split_to(src.len());
        self.decoder
            .feed(&data, |state| {
                result = Some(state);
            })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;

        Ok(result)
    }
}
