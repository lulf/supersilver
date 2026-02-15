#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::cmp::{max, min};
use drogue_device::*;
use linux_embedded_hal::Pin as PiPin;
use rotary_encoder_hal::{Direction, Rotary};
use std::process::Command;
use sysfs_gpio::Direction as GpioDirection;

#[derive(Device)]
pub struct MyDevice {}

#[drogue::main]
async fn main(context: DeviceContext<MyDevice>) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    let rot_a = PiPin::new(5);
    let rot_b = PiPin::new(6);
    let button = PiPin::new(24);
    let led = [PiPin::new(16), PiPin::new(12), PiPin::new(25)];
    for l in led.iter() {
        l.export().expect("Error exporting led pin");
        l.set_direction(GpioDirection::Out)
            .expect("Error setting led direction");
    }

    button.export().expect("Error exporting pin button");
    button
        .set_direction(GpioDirection::In)
        .expect("Error setting pin direction button");
    button
        .set_active_low(true)
        .expect("Error setting active low for button");
    rot_a.export().expect("Error exporting pin rot_a");
    rot_a
        .set_direction(GpioDirection::In)
        .expect("Error setting pin direction rot_a");

    rot_b.export().expect("Error exporting pin rot_b");
    rot_b
        .set_direction(GpioDirection::In)
        .expect("Error setting pin direction rot_b");

    context.configure(MyDevice {});

    context.mount(|_| {});

    let mut pressed = false;
    let mut muted = false;
    let mut pos_muted = 0;
    let mut position: i16 = 0;
    let mut rotary = Rotary::new(rot_a, rot_b);
    loop {
        // Debounce button
        /*
        if button.get_value().unwrap() != 0 && !pressed {
            log::info!("Button pressed");
            pressed = true;
            // Check if we should be muted
            if !muted {
                log::info!("Muting");
                muted = true;
                set_volume(0);
                pos_muted = position;
            } else {
                log::info!("Unmuting");
                muted = false;
                position = pos_muted;
            }
        } else if button.get_value().unwrap() == 0 {
            pressed = false;
        }
        */

        // Check volume position
        if !muted {
            let old_position = position;
            match rotary.update().unwrap() {
                Direction::Clockwise => position = min(255, position + 5),
                Direction::CounterClockwise => position = max(0, position - 5),
                Direction::None => {}
            }
            if old_position != position {
                for i in 0..led.len() {
                    if i == (position as usize) % 3 {
                        led[i].set_value(1).unwrap();
                    } else {
                        led[i].set_value(0).unwrap();
                    }
                }
                log::info!("Position: {}", position);
                set_volume(position as u8);
            }
        }
    }
}

fn set_volume(level: u8) {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("amixer set DSPVolume {}", level))
        .output()
        .expect("failed to execute process");
    log::info!("{:#?}", output);
}
