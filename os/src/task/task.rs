//! Types related to task management

use super::TaskContext;
use spin::{
    Mutex,
    MutexGuard
};

#[derive(Debug)]
pub struct TaskControlBlock {
    inner: Mutex<TaskControlBlockInner>
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running(usize),
    Exited,
}

#[derive(Debug)]
pub struct TaskControlBlockInner{
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
}


impl TaskControlBlock{
    pub fn acquire_inner_lock(&self) -> MutexGuard<TaskControlBlockInner>{
        self.inner.lock()
    }

    pub fn new() -> Self{
        TaskControlBlock{
            inner: Mutex::new(TaskControlBlockInner{
                task_status: TaskStatus::UnInit,
                task_cx: TaskContext::zero_init(),
            })
        }
    }
}

impl TaskControlBlockInner{
    pub fn get_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.task_cx as *mut TaskContext
    }
}