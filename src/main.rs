#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use drogue_device::*;
use linux_embedded_hal::Pin as PiPin;
use rotary_encoder_hal::{Direction, Rotary};
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
    let led = [PiPin::new(16), PiPin::new(12), PiPin::new(25)];
    for l in led.iter() {
        l.export().expect("Error exporting led pin");
        l.set_direction(GpioDirection::Out)
            .expect("Error setting led direction");
    }

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

    let mut position = 0;
    let mut rotary = Rotary::new(rot_a, rot_b);
    loop {
        let old_pos = position;
        match rotary.update().unwrap() {
            Direction::Clockwise => position += 1,
            Direction::CounterClockwise => position -= 1,
            Direction::None => {}
        }
        if old_pos != position {
            for i in 0..led.len() {
                if i == position % 3 {
                    led[i].set_value(1).unwrap();
                } else {
                    led[i].set_value(0).unwrap();
                }
            }
            log::info!("Position: {}", position);
        }
        /*
        led_blue
            .set_value(value % 2)
            .expect("Error setting led value");

        // Send that completes immediately when message is enqueued
        a_addr.notify(SayHello("World")).unwrap();
        // Send that waits until message is processed
        b_addr.request(SayHello("You")).unwrap().await;

        value += 1;
        */
    }
}
