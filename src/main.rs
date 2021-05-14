#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicU32, Ordering};
use drogue_device::*;
use linux_embedded_hal::Pin as PiPin;

pub struct MyActor {
    name: &'static str,
    counter: Option<&'static AtomicU32>,
}

impl MyActor {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            counter: None,
        }
    }
}

impl Actor for MyActor {
    type Configuration = &'static AtomicU32;
    type Message<'a> = SayHello<'a>;
    type OnStartFuture<'a> = impl Future<Output = ()> + 'a;
    type OnMessageFuture<'a> = impl Future<Output = ()> + 'a;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.counter.replace(config);
    }

    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move { log::info!("[{}] started!", self.name) }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            let count = self.counter.unwrap().fetch_add(1, Ordering::SeqCst);
            log::info!("[{}] hello {}: {}", self.name, message.0, count);
        }
    }
}

pub struct SayHello<'m>(&'m str);

#[derive(Device)]
pub struct MyDevice {
    counter: AtomicU32,
    a: ActorContext<'static, MyActor>,
    b: ActorContext<'static, MyActor>,
}

#[drogue::main]
async fn main(context: DeviceContext<MyDevice>) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    let led_blue = PiPin::new(12);

    context.configure(MyDevice {
        counter: AtomicU32::new(0),
        a: ActorContext::new(MyActor::new("a")),
        b: ActorContext::new(MyActor::new("b")),
    });

    let (a_addr, b_addr) = context.mount(|device| {
        let a_addr = device.a.mount(&device.counter);
        let b_addr = device.b.mount(&device.counter);
        (a_addr, b_addr)
    });

    loop {
        time::Timer::after(time::Duration::from_secs(1)).await;
        // Send that completes immediately when message is enqueued
        a_addr.notify(SayHello("World")).unwrap();
        // Send that waits until message is processed
        b_addr.request(SayHello("You")).unwrap().await;
    }
}
