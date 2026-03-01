use defmt::{info, warn};
use embassy_rp::i2c::{Blocking, I2c};
use embassy_rp::peripherals::{I2C0, I2C1};
use embassy_time::{Delay, Timer};

use adafruit_seesaw::devices::RotaryEncoder;
use adafruit_seesaw::prelude::*;

type DriverLeft = SeesawDriver<I2c<'static, I2C1, Blocking>, Delay>;
type DriverRight = SeesawDriver<I2c<'static, I2C0, Blocking>, Delay>;

/// Wait for the SAMD09 on the encoder board to boot after power-on.
const BOOT_DELAY_MS: u64 = 250;

/// Polling interval for encoder position reads.
const POLL_INTERVAL_MS: u64 = 10;

/// Left rotary encoder task.
///
/// Reads the encoder on I2C1 (GP14/GP15) and logs position changes.
#[embassy_executor::task]
pub async fn encoder_left_task(i2c: I2c<'static, I2C1, Blocking>) {
    Timer::after_millis(BOOT_DELAY_MS).await;

    let driver = SeesawDriver::new(Delay, i2c);
    let mut encoder: RotaryEncoder<DriverLeft> =
        match RotaryEncoder::new_with_default_addr(driver).init() {
            Ok(enc) => enc,
            Err(_) => {
                warn!("Left encoder: init failed");
                return;
            }
        };

    info!("Left encoder ready (addr {=u8:#x})", encoder.addr());

    let mut last_pos: i32 = 0;
    loop {
        match encoder.position(0) {
            Ok(pos) if pos != last_pos => {
                info!("Left  position: {=i32}", pos);
                last_pos = pos;
            }
            Ok(_) => {}
            Err(_) => warn!("Left encoder: read error"),
        }
        Timer::after_millis(POLL_INTERVAL_MS).await;
    }
}

/// Right rotary encoder task.
///
/// Reads the encoder on I2C0 (GP0/GP1) and logs position changes.
#[embassy_executor::task]
pub async fn encoder_right_task(i2c: I2c<'static, I2C0, Blocking>) {
    Timer::after_millis(BOOT_DELAY_MS).await;

    let driver = SeesawDriver::new(Delay, i2c);
    let mut encoder: RotaryEncoder<DriverRight> =
        match RotaryEncoder::new_with_default_addr(driver).init() {
            Ok(enc) => enc,
            Err(_) => {
                warn!("Right encoder: init failed");
                return;
            }
        };

    info!("Right encoder ready (addr {=u8:#x})", encoder.addr());

    let mut last_pos: i32 = 0;
    loop {
        match encoder.position(0) {
            Ok(pos) if pos != last_pos => {
                info!("Right position: {=i32}", pos);
                last_pos = pos;
            }
            Ok(_) => {}
            Err(_) => warn!("Right encoder: read error"),
        }
        Timer::after_millis(POLL_INTERVAL_MS).await;
    }
}
