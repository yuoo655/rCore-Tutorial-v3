mod context;
mod switch;
mod task;
mod manager;
mod processor;
mod pid;
mod pool;
mod action;
mod signal;
mod kthread;

use crate::fs::{open_file, OpenFlags};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};
use alloc::{sync::Arc};
pub use pool::{add_task, fetch_task, add_task_first_time};
use lazy_static::*;
pub use context::TaskContext;
pub use signal::{SignalFlags, MAX_SIG};
pub use action::{SignalAction, SignalActions};
pub use processor::{
    run_tasks,
    current_task,
    current_user_token,
    current_trap_cx,
    take_current_task,
    schedule,
    hart_id,
    current_trap_cx_user_va
};
pub use pid::{
    PidHandle, pid_alloc, KernelStack,
    RecycleAllocator,
    ustack_bottom_from_pid,
    trap_cx_bottom_from_pid,
    kstack_alloc,
};
pub use manager::{
    PID2TCB,
    pid2task,
    remove_from_pid2task,
    insert_into_pid2task,
};

pub use kthread::{
    TgidHandle, kernel_tgid_alloc,
    kernel_stackful_coroutine_test,
    kthread_trap_cx_bottom_from_tid,
    kthread_stack_bottom_from_tid,
};

use spin::Mutex;

lazy_static! {
    pub static ref WAIT_LOCK: Mutex<()> = Mutex::new(());
}



pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = task_inner.get_task_cx_ptr();
    // let task_cx_ptr = task_inner.gets_task_cx_ptr();
    drop(task_inner);

    // jump to scheduling cycle
    // add_task(task);
    schedule(task_cx_ptr);
}


pub fn block_current_and_run_next() {
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = task_inner.get_task_cx_ptr();
    task_inner.task_status = TaskStatus::Blocking;
    drop(task_inner);

    schedule(task_cx_ptr);
}


pub fn exit_current_and_run_next(exit_code: i32) {
    
    // ++++++ hold initproc PCB lock here
    let mut initproc_inner = INITPROC.inner_exclusive_access();

    // take from Processor
    let task = take_current_task().unwrap();


    let pid = task.pid.0;
    let tgid = task.tgid;

    // **** hold current PCB lock
    let wl = WAIT_LOCK.lock();
    let mut inner = task.inner_exclusive_access();

    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;

    // Record exit code
    inner.exit_code = Some(exit_code);
    
    // main thread exit
    if pid == tgid {
        remove_from_pid2task(pid);

        //move child to initproc
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }

        //clean up children dealloc resources
        inner.children.clear();
        // deallocate user space
        inner.memory_set.recycle_data_pages();
        // deallocate fdtable
        inner.fd_table.clear();

    }
    // release initproc lock
    drop(initproc_inner);

    drop(inner);
    drop(task);
    drop(wl);
    // **** release current PCB lock

    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();

    warn!("exit_current_and_run_next schedule");
    schedule(&mut _unused as *mut TaskContext);
}

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

pub fn add_initproc() {
    add_task_first_time(INITPROC.clone());
}

pub fn check_signals_error_of_current() -> Option<(i32, &'static str)> {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.signals.check_error()
}

pub fn current_add_signal(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.signals |= signal;
    drop(task_inner);
}

fn call_kernel_signal_handler(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    match signal {
        SignalFlags::SIGSTOP => {
            task_inner.frozen = true;
            task_inner.signals ^= SignalFlags::SIGSTOP;
        }
        SignalFlags::SIGCONT => {
            if task_inner.signals.contains(SignalFlags::SIGCONT) {
                task_inner.signals ^= SignalFlags::SIGCONT;
                task_inner.frozen = false;
            }
        }
        _ => {
            task_inner.killed = true;
        }
    }
}

fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();

    let handler = task_inner.signal_actions.table[sig].handler;
    // change current mask
    task_inner.signal_mask = task_inner.signal_actions.table[sig].mask;
    // handle flag
    task_inner.handling_sig = sig as isize;
    task_inner.signals ^= signal;

    // backup trapframe
    let mut trap_ctx = task_inner.get_trap_cx();
    // let trap_cx_copy = trap_ctx.clone();
    task_inner.trap_ctx_backup = Some(*trap_ctx);
    
    // modify trapframe
    trap_ctx.sepc = handler;

    // put args (a0)
    trap_ctx.x[10] = sig;
}

fn check_pending_signals() {
    for sig in 0..(MAX_SIG + 1) {
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let signal = SignalFlags::from_bits(1 << sig).unwrap();
        if task_inner.signals.contains(signal) && (!task_inner.signal_mask.contains(signal)) {
            if task_inner.handling_sig == -1 {
                drop(task_inner);
                drop(task);
                if signal == SignalFlags::SIGKILL || signal == SignalFlags::SIGSTOP ||
                    signal == SignalFlags::SIGCONT || signal == SignalFlags::SIGDEF {
                        // signal is a kernel signal
                        call_kernel_signal_handler(signal);
                } else {
                    // signal is a user signal
                    call_user_signal_handler(sig, signal);
                    return;
                }
            } else {
                if !task_inner.signal_actions.table[task_inner.handling_sig as usize].mask.contains(signal) {
                    drop(task_inner);
                    drop(task);
                    if signal == SignalFlags::SIGKILL || signal == SignalFlags::SIGSTOP ||
                        signal == SignalFlags::SIGCONT || signal == SignalFlags::SIGDEF {
                            // signal is a kernel signal
                            call_kernel_signal_handler(signal);
                    } else {
                        // signal is a user signal
                        call_user_signal_handler(sig, signal);
                        return;
                    }
                }
            }
        }
    }
}

pub fn handle_signals() {
    check_pending_signals();
    loop {
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let frozen_flag = task_inner.frozen;
        let killed_flag = task_inner.killed;
        drop(task_inner);
        drop(task);
        if (!frozen_flag) || killed_flag {
            break;
        }
        check_pending_signals();
        suspend_current_and_run_next()
    }
}
