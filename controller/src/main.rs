#![no_std]
#![no_main]

mod encoder;
mod usb;

use core::cell::{Cell, RefCell};

use defmt::{info, unwrap};
use embassy_boot_rp::{
    AlignedBuffer, BlockingFirmwareUpdater, FirmwareUpdaterConfig, State as BootState,
};
use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::Async;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, Config};
use embassy_rp::peripherals::{DMA_CH0, USB};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::Duration;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb_dfu::consts::DfuAttributes;
use embassy_usb_dfu::{new_state, usb_dfu, FirmwareHandler, ResetImmediate, UsbDfuState};
use {defmt_rtt as _, panic_probe as _};

use embassy_time::Timer;
use encoder::{encoder_left_task, encoder_right_task};
use static_cell::StaticCell;
use supersilver_protocol::EncoderState;
use usb::{usb_task, usb_write_task};

const FLASH_SIZE: usize = 2 * 1024 * 1024;
type Flash = embassy_rp::flash::Flash<'static, embassy_rp::peripherals::FLASH, Async, FLASH_SIZE>;
type FlashMutex = Mutex<NoopRawMutex, RefCell<Flash>>;
type DfuPart = BlockingPartition<'static, NoopRawMutex, Flash>;

pub static ENCODER_STATE: Mutex<CriticalSectionRawMutex, Cell<EncoderState>> = Mutex::new(
    Cell::new(EncoderState {
        left: 0,
        right: 0,
        left_pressed: false,
        right_pressed: false,
    }),
);

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Supersilver controller starting");

    // Start watchdog with 8-second timeout
    let mut watchdog = Watchdog::new(p.WATCHDOG);
    watchdog.start(Duration::from_secs(8));

    // Power on left encoder: GP13 set high
    let pwr_left = Output::new(p.PIN_13, Level::High);
    // Power on right encoder: GP2 set high
    let pwr_right = Output::new(p.PIN_2, Level::High);

    // Left encoder: I2C1 on GP14 (SDA) / GP15 (SCL)
    let i2c_left = i2c::I2c::new_blocking(p.I2C1, p.PIN_15, p.PIN_14, Config::default());

    // Right encoder: I2C0 on GP0 (SDA) / GP1 (SCL)
    let i2c_right = i2c::I2c::new_blocking(p.I2C0, p.PIN_1, p.PIN_0, Config::default());

    //    // USB CDC ACM setup
    let driver = Driver::new(p.USB, Irqs);
    //
    let mut config = embassy_usb::Config::new(0x1209, 0x0001);
    config.manufacturer = Some("Supersilver");
    config.product = Some("Controller");
    config.serial_number = Some("001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 4096]> = StaticCell::new();

    let config_desc = CONFIG_DESC.init([0; 256]);
    let bos_desc = BOS_DESC.init([0; 256]);
    let msos_desc = MSOS_DESC.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 4096]);

    static CDC_STATE: StaticCell<State> = StaticCell::new();
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

    // Set up flash for DFU and mark boot successful (prevents bootloader rollback)
    let flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0, Irqs);
    static FLASH_CELL: StaticCell<FlashMutex> = StaticCell::new();
    let flash = FLASH_CELL.init(Mutex::new(RefCell::new(flash)));

    let config = FirmwareUpdaterConfig::from_linkerfile_blocking(flash, flash);
    static DFU_ALIGNED: StaticCell<AlignedBuffer<1>> = StaticCell::new();
    let aligned = DFU_ALIGNED.init(AlignedBuffer([0; 1]));
    let mut updater = BlockingFirmwareUpdater::new(config, &mut aligned.0);

    if unwrap!(updater.get_state()) != BootState::Boot {
        unwrap!(updater.mark_booted());
    }

    let state = new_state(
        updater,
        DfuAttributes::CAN_DOWNLOAD | DfuAttributes::WILL_DETACH,
        ResetImmediate,
    );
    static DFU_STATE: StaticCell<
        UsbDfuState<FirmwareHandler<'static, DfuPart, DfuPart, ResetImmediate, 4096>>,
    > = StaticCell::new();
    let state = DFU_STATE.init(state);
    usb_dfu(&mut builder, state, |_| {});

    let usb_dev = builder.build();

    spawner.spawn(unwrap!(usb_task(usb_dev)));
    spawner.spawn(unwrap!(usb_write_task(class)));
    spawner.spawn(unwrap!(encoder_left_task(pwr_left, i2c_left)));
    spawner.spawn(unwrap!(encoder_right_task(pwr_right, i2c_right)));
    spawner.spawn(unwrap!(watchdog_task(watchdog)));
}

/// Periodically feed the watchdog to prevent a reset.
#[embassy_executor::task]
async fn watchdog_task(mut watchdog: Watchdog) {
    loop {
        watchdog.feed(Duration::from_secs(8));
        Timer::after_secs(1).await;
    }
}
