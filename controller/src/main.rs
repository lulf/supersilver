#![no_std]
#![no_main]

mod encoder;
mod usb;

use core::cell::{Cell, RefCell};

use defmt::info;
use embassy_boot_rp::{AlignedBuffer, BlockingFirmwareUpdater, FirmwareUpdaterConfig};
use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, Config};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::blocking_mutex::Mutex;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb_dfu::consts::DfuAttributes;
use embassy_usb_dfu::ResetImmediate;
use {defmt_rtt as _, panic_probe as _};

use encoder::{encoder_left_task, encoder_right_task};
use supersilver_protocol::EncoderState;
use usb::{usb_task, usb_write_task};

const FLASH_SIZE: usize = 2 * 1024 * 1024;
type InternalFlash = Flash<'static, embassy_rp::peripherals::FLASH, Blocking, FLASH_SIZE>;
type FlashMutex = Mutex<NoopRawMutex, RefCell<InternalFlash>>;
type DfuPart = BlockingPartition<'static, NoopRawMutex, InternalFlash>;
type DfuControl = embassy_usb_dfu::Control<'static, DfuPart, DfuPart, ResetImmediate, 4096>;

pub static ENCODER_STATE: Mutex<CriticalSectionRawMutex, Cell<EncoderState>> =
    Mutex::new(Cell::new(EncoderState { left: 0, right: 0 }));

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Supersilver controller starting");

    // Set up flash for DFU and mark boot successful (prevents bootloader rollback)
    let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    static FLASH_CELL: static_cell::StaticCell<FlashMutex> = static_cell::StaticCell::new();
    let flash = FLASH_CELL.init(Mutex::new(RefCell::new(flash)));
    {
        let config = FirmwareUpdaterConfig::from_linkerfile_blocking(flash, flash);
        let mut aligned = AlignedBuffer([0; 1]);
        let mut updater = BlockingFirmwareUpdater::new(config, &mut aligned.0);
        updater.mark_booted().unwrap();
    }

    // Power on left encoder: GP13 set high
    let pwr_left = Output::new(p.PIN_13, Level::High);
    // Power on right encoder: GP2 set high
    let pwr_right = Output::new(p.PIN_2, Level::High);

    // Left encoder: I2C1 on GP14 (SDA) / GP15 (SCL)
    let i2c_left = i2c::I2c::new_blocking(p.I2C1, p.PIN_15, p.PIN_14, Config::default());

    // Right encoder: I2C0 on GP0 (SDA) / GP1 (SCL)
    let i2c_right = i2c::I2c::new_blocking(p.I2C0, p.PIN_1, p.PIN_0, Config::default());

    // USB CDC ACM setup
    let driver = Driver::new(p.USB, Irqs);

    let mut config = embassy_usb::Config::new(0x1209, 0x0001);
    config.manufacturer = Some("Supersilver");
    config.product = Some("Controller");
    config.serial_number = Some("001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static BOS_DESC: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static MSOS_DESC: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static CONTROL_BUF: static_cell::StaticCell<[u8; 4096]> = static_cell::StaticCell::new();

    let config_desc = CONFIG_DESC.init([0; 256]);
    let bos_desc = BOS_DESC.init([0; 256]);
    let msos_desc = MSOS_DESC.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 4096]);

    static CDC_STATE: static_cell::StaticCell<State> = static_cell::StaticCell::new();
    let cdc_state = CDC_STATE.init(State::new());

    let mut builder = embassy_usb::Builder::new(
        driver,
        config,
        config_desc,
        bos_desc,
        msos_desc,
        control_buf,
    );

    let class = CdcAcmClass::new(&mut builder, cdc_state, 64);

    // DFU interface — accepts firmware downloads directly while the application runs.
    // After a complete download, the device resets and the bootloader swaps the new firmware in.
    let updater_config = FirmwareUpdaterConfig::from_linkerfile_blocking(flash, flash);
    static DFU_ALIGNED: static_cell::StaticCell<AlignedBuffer<1>> = static_cell::StaticCell::new();
    let dfu_aligned = DFU_ALIGNED.init(AlignedBuffer([0; 1]));
    let updater = BlockingFirmwareUpdater::new(updater_config, &mut dfu_aligned.0);
    static DFU_CONTROL: static_cell::StaticCell<DfuControl> = static_cell::StaticCell::new();
    let dfu_handler = DFU_CONTROL.init(embassy_usb_dfu::Control::new(
        updater,
        DfuAttributes::CAN_DOWNLOAD,
        ResetImmediate,
    ));
    embassy_usb_dfu::usb_dfu(&mut builder, dfu_handler, |_| {});

    let usb_dev = builder.build();

    spawner.spawn(usb_task(usb_dev)).unwrap();
    spawner.spawn(usb_write_task(class)).unwrap();
    spawner.spawn(encoder_left_task(pwr_left, i2c_left)).unwrap();
    spawner.spawn(encoder_right_task(pwr_right, i2c_right)).unwrap();
}
