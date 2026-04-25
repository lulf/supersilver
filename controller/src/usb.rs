use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_usb::class::cdc_acm::CdcAcmClass;
use embassy_usb::UsbDevice;

use defmt::{info, warn};
use embassy_time::Timer;
use supersilver_protocol::EncoderState;

use crate::ENCODER_STATE;

/// Run the USB device stack.
#[embassy_executor::task]
pub async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) {
    usb.run().await;
}

/// Read encoder positions and send them over USB CDC ACM as COBS-framed postcard messages.
#[embassy_executor::task]
pub async fn usb_write_task(mut class: CdcAcmClass<'static, Driver<'static, USB>>) {
    let mut last = EncoderState { left: 0, right: 0 };

    loop {
        info!("USB wait connection");
        class.wait_connection().await;
        info!("USB CDC ACM connected");

        loop {
            let current = ENCODER_STATE.lock(|s| s.get());

            if current.left != last.left || current.right != last.right {
                info!(
                    "Sending encoder state: left={=i32}, right={=i32}",
                    current.left, current.right
                );
                last = current;

                let mut buf = [0u8; 32];
                match supersilver_protocol::encode(&current, &mut buf) {
                    Ok(len) => {
                        if class.write_packet(&buf[..len]).await.is_err() {
                            warn!("USB write error");
                            break;
                        }
                    }
                    Err(_) => warn!("postcard encode error"),
                }
            }

            Timer::after_millis(10).await;
        }
    }
}
