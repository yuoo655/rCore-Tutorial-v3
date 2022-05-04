mod context;
mod manager;
mod pid;
mod processor;
mod switch;
mod pool;
#[allow(clippy::module_inception)]
mod task;

use crate::fs::{open_file, OpenFlags};
use alloc::sync::Arc;
pub use context::TaskContext;
use lazy_static::*;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

pub use pid::{pid_alloc, KernelStack, PidHandle};
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
};
pub use pool::{add_task, fetch_task};
use spin::Mutex;


lazy_static! {
    pub static ref WAIT_LOCK: Mutex<()> = Mutex::new(());
}

pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = current_task().unwrap();
    let mut task_inner = task.acquire_inner_lock();
    let task_cx_ptr = task_inner.get_task_cx_ptr();
    // let task_cx_ptr = task_inner.gets_task_cx_ptr();
    drop(task_inner);

    // jump to scheduling cycle
    // add_task(task);
    schedule(task_cx_ptr);
}



pub fn exit_current_and_run_next(exit_code: i32) {
    
    // ++++++ hold initproc PCB lock here
    let mut initproc_inner = INITPROC.acquire_inner_lock();

    // take from Processor
    let task = take_current_task().unwrap();
    
    // **** hold current PCB lock
    let wl = WAIT_LOCK.lock();
    let mut inner = task.acquire_inner_lock();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    for child in inner.children.iter() {
        child.acquire_inner_lock().parent = Some(Arc::downgrade(&INITPROC));
        initproc_inner.children.push(child.clone());
    }
    drop(initproc_inner);
    // ++++++ release parent PCB lock here

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    drop(inner);
    // **** release current PCB lock
    // drop task manually to maintain rc correctly
    drop(task);
    drop(wl);

    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();

    warn!("exit_current_and_run_next schedule");
    schedule(&mut _unused as *mut TaskContext);
}

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("usertests", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}
