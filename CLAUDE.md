# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Supersilver is a dual rotary encoder volume controller. It consists of:

- **`controller/`** — `#![no_std]` RP2040 firmware using Embassy async runtime. Reads two Adafruit I2C QT Rotary Encoders (seesaw) and sends encoder positions over USB CDC ACM as COBS-framed postcard messages.
- **`protocol/`** — Shared `no_std` message types (`EncoderState`) with COBS-framed postcard encode/decode. Used by both controller firmware and host.
- **`host/`** — Tokio-based host application that reads encoder data from USB serial port.
- **`parts/knobplate/`** — 3D printable parts generated with `vcad` (programmatic CAD).

These are independent Cargo projects (no workspace).

## Build Commands

### Controller firmware (requires `thumbv6m-none-eabi` target and probe-rs)
```sh
cd controller
cargo build --release          # build firmware
cargo run --release            # flash and run via probe-rs
DEFMT_LOG=info cargo run --release  # control log verbosity
```

### DFU bootloader (flash once via probe-rs, then use DFU for updates)
```sh
cd controller/bootloader
cargo build --release          # build bootloader
cargo run --release            # flash bootloader via probe-rs
```

### DFU firmware update (after bootloader is flashed)
```sh
# From host: trigger DFU mode, then flash new firmware
dfu-util -d 1209:0001 -D controller.bin   # upload firmware via USB DFU
```

### Protocol library
```sh
cd protocol
cargo test                     # run unit tests (round-trip encode/decode)
```

### Host application
```sh
cd host
cargo build
cargo run -- /dev/tty.usbmodemXXXX   # pass serial port as argument
```

### 3D parts
```sh
cd parts/knobplate
cargo run --release            # generates .stl files
```

## Architecture

### DFU Boot Flow

The system uses embassy-boot for firmware updates over USB DFU:

1. **Bootloader** (`controller/bootloader/`) — flashed once via probe-rs, lives in first 48K of flash
2. On boot: reads state partition, if `DfuDetach` → exposes USB DFU device for firmware download; if `Swap` → swaps active/DFU partitions; otherwise → boots application
3. **Application** includes DFU runtime interface alongside CDC ACM. Host sends DFU detach → app marks state and resets → bootloader enters DFU mode
4. After successful DFU download, bootloader marks swap and resets → swaps new firmware into active partition

Flash layout: BOOT2 (256B) | Bootloader (48K) | State (4K) | Active (512K) | DFU (516K)

### Controller Tasks

The controller firmware runs four async tasks on a single-threaded Embassy executor:

1. **`encoder_left_task`** — polls left encoder via I2C1 (GP14/GP15, powered by GP13)
2. **`encoder_right_task`** — polls right encoder via I2C0 (GP0/GP1, powered by GP2)
3. **`usb_task`** — runs the USB device stack
4. **`usb_write_task`** — reads shared `ENCODER_STATE` and sends changes over USB CDC ACM

Encoder tasks share state with the USB write task through a `Mutex<CriticalSectionRawMutex, Cell<EncoderState>>` global.

The protocol uses postcard serialization with COBS framing (0x00 sentinel byte). The `Decoder` is a streaming accumulator that handles split reads across feed calls.

## Key Conventions

- Encoders need a 250ms boot delay after power-on for the SAMD09 to start
- Power GPIO pins must be kept alive (moved into tasks) or the encoders lose power
- `memory.x` must exist at `controller/` root (and `controller/bootloader/`) for linker scripts
- The application's `memory.x` must NOT include `-Tlink-rp.x` (bootloader handles boot2)
- Application must call `mark_booted()` on startup to prevent bootloader rollback
- The controller `.cargo/config.toml` sets the build target and probe-rs runner automatically
- All encoder/USB types require `'static` lifetime since they're moved into spawned Embassy tasks
