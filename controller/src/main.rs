#![no_std]
#![no_main]

mod encoder;

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, Config};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

use encoder::{encoder_left_task, encoder_right_task};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Supersilver controller starting");

    // Power on left encoder: GP13 set high
    let _pwr_left = Output::new(p.PIN_13, Level::High);
    // Power on right encoder: GP2 set high
    let _pwr_right = Output::new(p.PIN_2, Level::High);

    // Left encoder: I2C1 on GP14 (SDA) / GP15 (SCL)
    let i2c_left = i2c::I2c::new_blocking(p.I2C1, p.PIN_15, p.PIN_14, Config::default());

    // Right encoder: I2C0 on GP0 (SDA) / GP1 (SCL)
    let i2c_right = i2c::I2c::new_blocking(p.I2C0, p.PIN_1, p.PIN_0, Config::default());

    spawner.spawn(encoder_left_task(i2c_left)).unwrap();
    spawner.spawn(encoder_right_task(i2c_right)).unwrap();

    // Keep main alive so the power Output pins are not dropped
    loop {
        Timer::after_secs(1).await;
    }
}
