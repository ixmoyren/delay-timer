//! DelayTimer is a cyclic task manager with latency properties,
//! based on an internal event manager and task scheduler,
//! and supported by the runtime provided by smol,
//! which makes it easy to manage asynchronous/synchronous/scripted cyclic tasks.
//!
//! # DelayTimer
//!
//! User applications can be served through the lib used by DelayTimer:
//!
//! 1. Mission deployment.
#![feature(split_inclusive)]
#![feature(drain_filter)]
#![feature(test)]
extern crate test;
pub mod delay_timer;
pub mod generate_fn_macro;
pub mod timer;
pub mod utils;

extern crate lazy_static;

pub use generate_fn_macro::*;
pub use smol::future as future_lite;
pub use smol::spawn as async_spawn;
pub use smol::unblock as unblock_spawn;

#[cfg(test)]
mod tests {
    use smol::{channel::unbounded, future::block_on};
    use std::sync::{atomic::AtomicU64, Arc};
    use waitmap::WaitMap;

    use crate::{
        delay_timer::{DelayTimer, SharedHeader},
        timer::{
            event_handle::EventHandle,
            runtime_trace::task_handle::DelayTaskHandler,
            task::{Frequency, TaskBuilder},
            timer_core::{Timer, TimerEvent, TimerEventSender, DEFAULT_TIMER_SLOT_COUNT},
        },
        utils::functions::{create_default_delay_task_handler, create_delay_task_handler},
    };

    use test::Bencher;

    #[bench]
    fn bench_task_spwan(b: &mut Bencher) {
        let body = move || create_default_delay_task_handler();

        let mut task_builder = TaskBuilder::default();
        task_builder.set_frequency(Frequency::CountDown(1, "0/10 * * * * * *"));
        task_builder.set_maximum_running_time(5);
        task_builder.set_task_id(1);

        b.iter(|| task_builder.spawn(body.clone()));
    }

    #[bench]
    fn bench_maintain_task(b: &mut Bencher) {
        let (timer_event_sender, timer_event_receiver) = unbounded::<TimerEvent>();
        let shared_header = SharedHeader::default();
        let mut timer = Timer::new(timer_event_sender.clone(), shared_header);

        let body = move || create_default_delay_task_handler();

        let mut task_builder = TaskBuilder::default();
        task_builder.set_frequency(Frequency::CountDown(1, "0/10 * * * * * *"));
        task_builder.set_maximum_running_time(5);
        task_builder.set_task_id(1);

        let mut tasks = Vec::with_capacity(1000000);

        for _ in 0..1000000 {
            tasks.push(task_builder.spawn(body));
        }

        b.iter(|| block_on(timer.maintain_task(tasks.pop().unwrap(), 1, 1, 1)));
    }
}
