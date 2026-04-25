use defmt::{info, warn};
use embassy_rp::gpio::Output;
use embassy_rp::i2c::{Blocking, I2c};
use embassy_rp::peripherals::{I2C0, I2C1};
use embassy_time::{Delay, Timer};
use embedded_hal::i2c::I2c as I2cInterface;

use adafruit_seesaw::devices::RotaryEncoder;
use adafruit_seesaw::prelude::*;

use crate::ENCODER_STATE;

/// Wait for the SAMD09 on the encoder board to boot after power-on.
const BOOT_DELAY_MS: u64 = 250;

/// Polling interval for encoder position reads.
const POLL_INTERVAL_MS: u64 = 10;

#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
}

/// Left rotary encoder task.
///
/// Reads the encoder on I2C1 (GP14/GP15) and logs position changes.
#[embassy_executor::task]
pub async fn encoder_left_task(_power: Output<'static>, i2c: I2c<'static, I2C1, Blocking>) {
    run_encoder(i2c, Side::Left, "left").await;
}

/// Right rotary encoder task.
///
/// Reads the encoder on I2C0 (GP0/GP1) and logs position changes.
#[embassy_executor::task]
pub async fn encoder_right_task(_power: Output<'static>, i2c: I2c<'static, I2C0, Blocking>) {
    run_encoder(i2c, Side::Right, "right").await;
}

async fn run_encoder<I: I2cInterface>(i2c: I, side: Side, name: &'static str) {
    Timer::after_millis(BOOT_DELAY_MS).await;

    let driver = SeesawDriver::new(Delay, i2c);
    let mut encoder: RotaryEncoder<SeesawDriver<I, Delay>> =
        match RotaryEncoder::new_with_default_addr(driver).init() {
            Ok(enc) => enc,
            Err(_) => {
                warn!("[{}]: init failed", name);
                return;
            }
        };

    info!("[{}] ready (addr {=u8:#x})", name, encoder.addr());

    let mut last_pos: i32 = 0;
    let mut last_pressed: bool = false;
    loop {
        match encoder.position(0) {
            Ok(pos) if pos != last_pos => {
                info!("[{}] position: {=i32}", name, pos);
                last_pos = pos;
                ENCODER_STATE.lock(|s| {
                    let mut state = s.get();
                    match side {
                        Side::Left => state.left = pos,
                        Side::Right => state.right = pos,
                    }
                    s.set(state);
                });
            }
            Ok(_) => {}
            Err(_) => warn!("[{}] encoder: read error", name),
        }

        // seesaw `digital_read` inverts the raw GPIO level, so for an InputPullup
        // button it returns true when pressed (pin LOW) and false when released.
        match encoder.button(0) {
            Ok(pressed) => {
                if pressed != last_pressed {
                    info!("[{}] button: {=bool}", name, pressed);
                    last_pressed = pressed;
                    ENCODER_STATE.lock(|s| {
                        let mut state = s.get();
                        match side {
                            Side::Left => state.left_pressed = pressed,
                            Side::Right => state.right_pressed = pressed,
                        }
                        s.set(state);
                    });
                }
            }
            Err(_) => warn!("[{}] button: read error", name),
        }

        Timer::after_millis(POLL_INTERVAL_MS).await;
    }
}
