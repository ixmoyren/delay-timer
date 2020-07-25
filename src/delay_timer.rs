use super::timer::{
    task::Task,
    timer_core::{Timer, TimerEvent, TimerEventSender},
};
use anyhow::{Context, Result};
use std::sync::mpsc::channel;
use threadpool::ThreadPool;

//FIXME: Relace Generics into Enum.
//Backupground Description :
// I have a Type Task<T> that's a smallest type. (Task<T>{task_id:i32, body:Box<dyn Fn -> T>})
// Slot<T> => Timer<T> => DelayTimer<T> all of these type base on Task<T>.
// That's the problem , I want Unified handling Task<Child>  Task<SmolTask>  in Timer
// But, If i store Task<Child> In Timer filst  I can't store Task<SmolTask> in next time,
// Because Rust compiler get T is Child so DelayTimer<T> => DelayTimer<Child>
// Then any other T instance can't store.
// So, I plan to Replace Generics into Enum.
pub struct DelayTimer {
    timer_event_sender: TimerEventSender,
}

//TODO:来一个hashMqp  task_id => child-handle-linklist
//可以取消任务，child-handle 可以是进程句柄 - 也可以是异步句柄， 用linklist 是因为，可能任务支持同时多个并行
impl DelayTimer {
    pub fn new() -> DelayTimer {
        let (timer_event_sender, timer_event_receiver) = channel::<TimerEvent>();
        let mut timer = Timer::new(timer_event_receiver);

        // Use threadpool can replenishes the pool if any worker threads panic.
        let pool = ThreadPool::new(1);

        //sync schedule
        // thread::spawn(move || timer.schedule());

        pool.execute(move || {
            smol::run(async {
                timer.async_schedule().await;
            })
        });

        DelayTimer { timer_event_sender }
    }

    pub fn add_task(&mut self, task: Task) -> Result<()> {
        self.seed_timer_event(TimerEvent::AddTask(Box::new(task)))
    }

    pub fn remove_task(&mut self, task_id: u32) -> Result<()> {
        self.seed_timer_event(TimerEvent::RemoveTask(task_id))
    }

    pub fn cancel_task(&mut self, task_id: u32) -> Result<()> {
        self.seed_timer_event(TimerEvent::CancelTask(task_id))
    }

    fn seed_timer_event(&mut self, event: TimerEvent) -> Result<()> {
        self.timer_event_sender
            .send(event)
            .with_context(|| format!("Failed Send Event from seed_timer_event"))
    }
}

impl Default for DelayTimer {
    fn default() -> Self {
        Self::new()
    }
}
