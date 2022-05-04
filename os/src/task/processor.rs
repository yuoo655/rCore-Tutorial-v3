use super::{
    TaskControlBlock,
    TaskContext,
    TaskStatus,
    fetch,
    add_task
};
use super::switch::__switch;

use core::cell::RefCell;
use alloc::sync::Arc;
use core::arch::asm;
use crate::trap::TrapContext;

use lazy_static::*;

lazy_static! {
    /// cpu instance
    pub static ref PROCESSORS: [Processor; 4] = Default::default();
}

pub struct Processor {
    inner: RefCell<ProcessorInner>,
}

pub struct ProcessorInner {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}

unsafe impl Sync for Processor {}

impl Default for Processor {
    fn default() -> Self {
        Self {
            inner: RefCell::new(ProcessorInner {
                current: None,
                idle_task_cx: Default::default(),
            }),
        }
    }
}

impl Processor {
    pub fn new() -> Self{
        Processor{
            inner: RefCell::new(ProcessorInner::new())
        }
    }
    pub fn get_idle_task_cx_ptr(&self) -> *mut TaskContext {
        let mut inner = self.inner.borrow_mut();
        &mut inner.idle_task_cx as *mut TaskContext
    }
    
    pub fn run(&self) {
        loop {
            if let Some(task) = fetch() {
                self.run_next(task);
                self.suspend_current();
            }
        }
    }

    pub fn run_next(&self, task: Arc<TaskControlBlock>) {
        //each cpu has idle task
        let idle_task_cx_ptr = self.get_idle_task_cx_ptr();

        let mut task_inner = task.acquire_inner_lock();

        let next_task_cx_ptr = task_inner.get_task_cx_ptr();

        task_inner.task_status = TaskStatus::Running(hart_id());

        drop(task_inner);
        self.inner.borrow_mut().current = Some(task);
        unsafe{
            __switch(idle_task_cx_ptr, next_task_cx_ptr);
        }

    }

    pub fn suspend_current(&self) {
        if let Some(task) = take_current_task() {
            let mut task_inner = task.acquire_inner_lock();
            task_inner.task_status = TaskStatus::Ready;
            drop(task_inner);


            add_task(task);
        }

    }
    pub fn take_current(&self) -> Option<Arc<TaskControlBlock>> {
        self.inner.borrow_mut().current.take()
    }

    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.inner.borrow().current.as_ref().map(|task| Arc::clone(task))
    }
}

/// take current cpu task move ownership
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSORS[hart_id()].take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSORS[hart_id()].current()
}

/// get current cpu task TrapContext
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().unwrap().acquire_inner_lock().get_trap_cx()
}

/// get current cpu task user memoryset root ppn
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.acquire_inner_lock().get_user_token();
    token
}

impl ProcessorInner {
    pub fn new() -> Self{
        ProcessorInner {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
}


pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}

