use crate::{
    mm::kernel_token,
    task::{add_task, current_task, TaskControlBlock},
    trap::{trap_handler, TrapContext},
};
use crate::task::{
    add_task_first_time,
    pid2task,
};


pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.new_user_thread(entry, arg);
    let new_pid = new_task.pid.0;

    // add new task to scheduler
    add_task_first_time(new_task);
    drop(current_task);
    new_pid as isize 
}

pub fn sys_gettid() -> isize {
    let task = current_task().unwrap();
    task.pid.0 as isize
}

/// thread does not exist, return -1
/// thread has not exited yet, return -2
/// otherwise, return thread's exit code
pub fn sys_waittid(tid: usize) -> i32 {
    let task = current_task().unwrap();

    // a thread cannot wait for itself
    if task.pid.0 == task.tgid {
        return -1;
    }
    let mut exit_code: Option<i32> = None;

    if let Some(waited_task) = pid2task(tid) {
        if let Some(waited_exit_code) = waited_task.inner_exclusive_access().exit_code {
            exit_code = Some(waited_exit_code);
        }
    } else {
        // waited thread does not exist
        return -1;
    }
    1
}
