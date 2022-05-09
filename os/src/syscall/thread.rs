use crate::{
    mm::kernel_token,
    task::{add_task, current_task, TaskControlBlock},
    trap::{trap_handler, TrapContext},
};
use crate::task::{
    add_task_first_time,
    pid2task,
    trap_cx_bottom_from_pid,
    remove_from_pid2task
};


pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    let current_task = current_task().unwrap();
    let current_pid = current_task.pid.0;

    let new_task = current_task.new_user_thread(entry, arg, current_pid);

    let new_pid = new_task.pid.0;

    let cx = trap_cx_bottom_from_pid(new_pid);
    // println!("cx: {:#x}", cx);
    // add new task to scheduler
    add_task_first_time(new_task.clone());
    drop(current_task);
    drop(new_task);

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


    let tgid = task.tgid;

    // parent pid
    let pid = task.pid.0;


    println!("wait pid:{}, current: pid={} tgid={}", tid,pid,tgid);

    let waited_task = pid2task(tid);

    // a thread cannot wait for itself
    if pid == tgid {
        return -1;
    }

    
    let mut exit_code: Option<i32> = None;

    if let Some(waited_task) = waited_task {
        
        if let Some(waited_exit_code) = waited_task.inner_exclusive_access().get_exit_code(){
            exit_code = Some(waited_exit_code);
        }
    } else {
        // waited thread does not exist
        return -1;
    }
    if let Some(exit_code) = exit_code {

        remove_from_pid2task(tid);
        exit_code
    } else {
        // waited thread has not exited
        -2
    }

}
