TARGET_DIR = target/thumbv6m-none-eabi/release

.PHONY: all firmware bootloader clean flash-bootloader flash-firmware

all: firmware.bin

bootloader:
	cd bootloader && cargo build --release

firmware:
	cargo build --release

firmware.bin: firmware
	rust-objcopy -O binary $(TARGET_DIR)/controller firmware.bin

flash-bootloader: bootloader
	cd bootloader && cargo run --release

flash-firmware: firmware
	cargo run --release

clean:
	cargo clean
	cd bootloader && cargo clean
	rm -f firmware.bin
