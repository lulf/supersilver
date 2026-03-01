# Supersilver Controller

Firmware for the Supersilver controller — a Raspberry Pi Pico (RP2040) reading two
[Adafruit I2C QT Rotary Encoders](https://www.adafruit.com/product/4991) for volume control.

## Hardware

| Signal        | Pin  | Notes                        |
|---------------|------|------------------------------|
| Left power    | GP13 | Set HIGH to enable encoder   |
| Left SDA      | GP14 | I2C1                         |
| Left SCL      | GP15 | I2C1                         |
| Right power   | GP2  | Set HIGH to enable encoder   |
| Right SDA     | GP0  | I2C0                         |
| Right SCL     | GP1  | I2C0                         |

Both encoders use the default I2C address `0x36` (Adafruit seesaw default).
Since they are on separate buses there is no address conflict.

## Dependencies

- [Embassy](https://embassy.dev/) — async embedded runtime (`embassy-executor`, `embassy-rp`, `embassy-time`)
- [adafruit-seesaw](https://crates.io/crates/adafruit-seesaw) — seesaw device driver
- [defmt](https://defmt.rs/) + [defmt-rtt](https://crates.io/crates/defmt-rtt) — logging over RTT
- [probe-rs](https://probe.rs/) — flashing and running (`cargo run`)

## Building

```sh
cargo build --release
```

## Flashing / Running

Connect a debug probe (e.g. another Pico running [picoprobe](https://github.com/raspberrypi/picoprobe))
then:

```sh
cargo run --release
```

This uses `probe-rs run --chip RP2040` as configured in `.cargo/config.toml`.

Set `DEFMT_LOG` to control log verbosity (default: `debug`):

```sh
DEFMT_LOG=info cargo run --release
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
