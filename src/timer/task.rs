use super::runtime_trace::task_handle::DelayTaskHandler;
use cron_clock::schedule::{Schedule, ScheduleIteratorOwned};
use cron_clock::Utc;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

//TaskMark is use to remove/stop the task.
pub struct TaskMark {
    task_id: u32,
    slot_mark: u32,
}

pub enum TaskType {
    AsyncType,
    SyncType,
}

impl TaskMark {
    fn new(task_id: u32) -> Self {
        TaskMark {
            task_id,
            slot_mark: 0,
        }
    }

    pub fn get_slot_mark(&self) -> u32 {
        self.slot_mark
    }
}

//TODO: Maybe that's can optimize.(We can add/del/set TaskMark in Timer.async_schedule)
//TASKMAP is use to storage all TaskMark for check.
lazy_static! {
    pub static ref TASKMAP: Mutex<HashMap<u32, TaskMark>> = {
        let m = HashMap::new();
        Mutex::new(m)
    };
}

#[derive(Debug)]
pub enum Frequency {
    Once(&'static str),
    Repeated(&'static str),
    CountDown(u32, &'static str),
}

pub enum FrequencyInner {
    Repeated(ScheduleIteratorOwned<Utc>),
    CountDown(u32, ScheduleIteratorOwned<Utc>),
}

impl FrequencyInner {
    fn residual_time(&self) -> u32 {
        match self {
            FrequencyInner::Repeated(_) => u32::MAX,
            FrequencyInner::CountDown(ref time, _) => *time,
        }
    }

    fn next_alarm_timestamp(&mut self) -> i64 {
        //TODO:handle error
        match self {
            FrequencyInner::CountDown(_, ref mut clock) => clock.next().unwrap().timestamp(),
            FrequencyInner::Repeated(ref mut clock) => clock.next().unwrap().timestamp(),
        }
    }

    #[warn(unused_parens)]
    fn down_count(&mut self) {
        match self {
            FrequencyInner::CountDown(ref mut exec_count, _) => {
                *exec_count -= 1u32;
            }
            FrequencyInner::Repeated(_) => {}
        };
    }

    fn is_down_over(&mut self) -> bool {
        match self {
            FrequencyInner::CountDown(0, _) => false,
            _ => true,
        }
    }
}

#[derive(Debug, Default)]
pub struct TaskBuilder {
    frequency: Option<Frequency>,
    task_id: u32,
}

//TASK 执行完了，支持找新的Slot
type SafeBoxFn = Box<dyn Fn() -> Box<dyn DelayTaskHandler> + 'static + Send + Sync>;
pub struct Task {
    pub task_id: u32,
    frequency: FrequencyInner,
    pub body: SafeBoxFn,
    cylinder_line: u32,
    valid: bool,
}

//bak type BoxFn
// type BoxFn = Box<dyn Fn(i32) -> Pin<Box<dyn Future<Output = i32>>>>;

enum RepeatType {
    Num(u32),
    Always,
}

impl<'a> TaskBuilder {
    pub fn set_frequency(&mut self, frequency: Frequency) {
        self.frequency = Some(frequency);
    }
    pub fn set_task_id(&mut self, task_id: u32) {
        self.task_id = task_id;
    }

    pub fn spawn<F>(self, body: F) -> Task
    where
        F: Fn() -> Box<dyn DelayTaskHandler> + 'static + Send + Sync,
    {
        let frequency_inner;

        let mut m = TASKMAP.lock().unwrap();
        m.insert(self.task_id, TaskMark::new(self.task_id));

        //我需要将 使用task_id关联任务，放到一个全局的hash表
        //两个作用，task_id 跟 Task 一一对应
        //在hash表上会存着，Task当前处在的Slot

        //通过输入的模式匹配，表达式与重复类型
        let (expression_str, repeat_type) = match self.frequency.unwrap() {
            Frequency::Once(expression_str) => (expression_str, RepeatType::Num(1)),
            Frequency::Repeated(expression_str) => (expression_str, RepeatType::Always),
            Frequency::CountDown(exec_count, expression_str) => {
                (expression_str, RepeatType::Num(exec_count))
            }
        };

        //构建时间迭代器
        let schedule = Schedule::from_str(expression_str).unwrap();
        let taskschedule = schedule.upcoming_owned(Utc);

        //根据重复类型，构建TaskFrequencyInner模式
        frequency_inner = match repeat_type {
            RepeatType::Always => FrequencyInner::Repeated(taskschedule),
            RepeatType::Num(repeat_count) => FrequencyInner::CountDown(repeat_count, taskschedule),
        };

        Task::new(self.task_id, frequency_inner, Box::new(body))
    }
}

impl Task {
    pub fn new(task_id: u32, frequency: FrequencyInner, body: SafeBoxFn) -> Task {
        Task {
            task_id,
            frequency,
            body,
            cylinder_line: 0,
            valid: true,
        }
    }

    //swap slot loction ,do this
    //down_count_and_set_vaild,will return new vaild status.
    pub fn down_count_and_set_vaild(&mut self) -> bool {
        self.down_count();
        self.set_valid_by_count_down();
        self.is_valid()
    }

    //down_exec_count
    fn down_count(&mut self) {
        self.frequency.down_count();
    }

    //set_valid_by_count_down
    fn set_valid_by_count_down(&mut self) {
        self.valid = self.frequency.is_down_over();
    }

    pub fn set_cylinder_line(&mut self, cylinder_line: u32) {
        self.cylinder_line = cylinder_line;
    }

    //single slot foreach do this.
    //sub_cylinder_line
    //return is can_running?
    pub fn sub_cylinder_line(&mut self) -> bool {
        self.cylinder_line -= 1;
        self.is_can_running()
    }

    pub fn check_arrived(&mut self) -> bool {
        if self.cylinder_line == 0 {
            return self.is_can_running();
        }
        self.sub_cylinder_line()
    }

    //check is ready
    pub fn is_already(&self) -> bool {
        self.cylinder_line == 0
    }

    //is_can_running
    pub fn is_can_running(&self) -> bool {
        if self.is_valid() {
            return self.is_already();
        }
        false
    }

    //is_valid
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    //get_next_exec_timestamp
    pub fn get_next_exec_timestamp(&mut self) -> i64 {
        self.frequency.next_alarm_timestamp()
    }
}
