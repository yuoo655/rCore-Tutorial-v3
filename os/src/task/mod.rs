//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
mod processor;

#[allow(clippy::module_inception)]
mod task;

// use crate::sync::UPSafeCell;
use lazy_static::*;
// use switch::__switch;
use task::{TaskControlBlock, TaskStatus};
use core::arch::asm;
use alloc::sync::Arc;
use spin::Mutex;
use alloc::collections::VecDeque;

pub use context::TaskContext;
pub use processor::PROCESSORS;

pub use self::processor::{take_current_task,current_trap_cx,current_user_token};
use self::switch::__switch;



lazy_static!{
    /// A global run queue controls all the tasks in the operating system.
    pub static ref GLOBALTASKRUNQUEUE: Mutex<TaskManager> = Mutex::new(TaskManager::new());
}

/// The task manager, where all the tasks are managed.
pub struct TaskManager {
    ready_queue : VecDeque<Arc<TaskControlBlock>>,
}

/// impl of task manager
impl TaskManager {

    /// Create a new task manager. 
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }

    ///add task
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }

    /// fetch task
    pub fn fetch_task(&mut self) -> Option<Arc<TaskControlBlock>> {
        // May need to concern affinity
        self.ready_queue.pop_front()
    }
}

/// add all user task
pub fn add_user_tasks(){
    for i in 0..5 {
        let task = Arc::new(TaskControlBlock::new(crate::loader::get_app_data(i), i));
        add_task(task);
    }
    println!("add user tasks done");
}

/// add task
pub fn add_task(task: Arc<TaskControlBlock>) {
    GLOBALTASKRUNQUEUE.lock().add(task);
}
/// run task
pub fn run_tasks(){
    println!("[hart {}] run tasks", hart_id());
    PROCESSORS[hart_id()].run();
}

/// fetch task 
pub fn fetch() -> Option<Arc<TaskControlBlock>> {
    GLOBALTASKRUNQUEUE.lock().fetch_task()
}

/// exit current task switch to idle task
pub fn exit_current_and_run_next(){
    let task = take_current_task().unwrap();
    let mut task_inner = task.acquire_inner_lock();
    task_inner.task_status = TaskStatus::Exited;
    drop(task_inner);
    drop(task);
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

/// switch to idle
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let idle_task_cx_ptr = PROCESSORS[hart_id()].get_idle_task_cx_ptr();
    unsafe{
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

/// suspend current task switch to idle task
pub fn suspend_current_and_run_next(){
    let task = take_current_task().unwrap();
    let mut task_inner = task.acquire_inner_lock();
    task_inner.task_status = TaskStatus::Ready;
    let task_cx_ptr = task_inner.get_task_cx_ptr();
    drop(task_inner);
    add_task(task);
    schedule(task_cx_ptr);
}


/// get current cpu id
pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}
