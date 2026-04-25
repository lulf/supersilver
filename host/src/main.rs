use std::process::Command;

use nusb::io::EndpointRead;
use nusb::transfer::{Bulk, In};
use supersilver_protocol::Decoder;
use tokio::io::AsyncReadExt;

const VID: u16 = 0x1209;
const PID: u16 = 0x0001;
// embassy-usb CDC-ACM lays out: iface 0 = comm, iface 1 = data (bulk).
const DATA_INTERFACE: u8 = 1;
const BULK_IN_EP: u8 = 0x82;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let test_mode = args.iter().any(|a| a == "--test");
    let control = args
        .windows(2)
        .find(|w| w[0] == "--control")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "DSPVolume".to_string());

    let device_info = nusb::list_devices()
        .await?
        .find(|d| d.vendor_id() == VID && d.product_id() == PID)
        .ok_or("Supersilver controller not found (VID 0x1209 PID 0x0001)")?;

    println!("Opening {:04x}:{:04x}", VID, PID);
    let device = device_info.open().await?;

    // CDC kernel driver claims this interface on Linux; detach it first.
    #[cfg(target_os = "linux")]
    let interface = device.detach_and_claim_interface(DATA_INTERFACE).await?;
    #[cfg(not(target_os = "linux"))]
    let interface = device.claim_interface(DATA_INTERFACE).await?;

    let endpoint = interface.endpoint::<Bulk, In>(BULK_IN_EP)?;
    let mut reader = EndpointRead::new(endpoint, 4096);

    println!("Listening for encoder state...");

    let mut decoder = Decoder::new();
    let mut prev_right: Option<i32> = None;
    let mut prev_right_pressed: bool = false;
    let mut volume: i32 = 50;
    let mut muted: bool = false;
    let mut saved_volume: Option<String> = None;
    let mut buf = [0u8; 64];

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            continue;
        }

        decoder.feed(&buf[..n], |state| {
            println!(
                "left: {:>4} ({})  right: {:>4} ({})",
                state.left,
                if state.left_pressed { "down" } else { "up" },
                state.right,
                if state.right_pressed { "down" } else { "up" },
            );

            if let Some(prev) = prev_right {
                let delta = state.right - prev;
                if delta != 0 {
                    if test_mode {
                        volume = (volume + delta).clamp(0, 100);
                        println!("volume: {volume}%");
                    } else {
                        // Rotating after a mute drops the saved level — the
                        // user has chosen a new level, so a later press should
                        // mute from there, not restore the stale value.
                        saved_volume = None;
                        adjust_volume(&control, delta);
                    }
                }
            }
            prev_right = Some(state.right);

            // Right encoder press toggles mute on the volume control.
            if state.right_pressed && !prev_right_pressed {
                if test_mode {
                    muted = !muted;
                    println!("muted: {muted}");
                } else if let Some(prev_level) = saved_volume.take() {
                    println!("unmute: restoring {prev_level}");
                    set_volume(&control, &prev_level);
                } else if let Some(current) = current_volume(&control) {
                    println!("mute: was {current}");
                    saved_volume = Some(current);
                    set_volume(&control, "0%");
                } else {
                    eprintln!("mute: failed to read current volume");
                }
            }
            prev_right_pressed = state.right_pressed;
        })?;
    }
}

fn adjust_volume(control: &str, delta: i32) {
    let step = format!("{}%", delta.unsigned_abs());
    let dir = if delta > 0 { "+" } else { "-" };
    let arg = format!("{step}{dir}");

    match Command::new("amixer").args(["sset", control, &arg]).output() {
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

/// Read the current volume from `amixer sget <control>`, looking for the first
/// `[NN%]` field. Returns the level as `"NN%"` so it can be passed straight back
/// to `amixer sset`.
fn current_volume(control: &str) -> Option<String> {
    let output = Command::new("amixer").args(["sget", control]).output().ok()?;
    if !output.status.success() {
        eprintln!(
            "amixer sget failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let start = text.find('[')? + 1;
    let rest = &text[start..];
    let end = rest.find("%]")?;
    let n: u32 = rest[..end].parse().ok()?;
    Some(format!("{n}%"))
}

fn set_volume(control: &str, level: &str) {
    match Command::new("amixer")
        .args(["sset", control, level])
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "amixer sset {level} failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        Err(e) => eprintln!("failed to run amixer: {e}"),
    }
}
