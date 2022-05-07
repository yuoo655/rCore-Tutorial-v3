use alloc::sync::Arc;
use alloc::vec::Vec;

use riscv::register::sstatus::{self, Sstatus, SPP};
use crate::trap::{
    TrapContext, trap_handler,trap_from_kernel
};
use crate::mm::{
    KERNEL_SPACE,
    kernel_token
};

use crate::config::{
    MEMORY_END, PAGE_SIZE,
};
use super::{
    TaskControlBlock,
    add_task_first_time,
    add_task,
    suspend_current_and_run_next,
    exit_current_and_run_next,
    TaskContext,
    schedule,
    RecycleAllocator,
    INITPROC,
    take_current_task,
    TaskStatus,
    WAIT_LOCK
};
use spin::Mutex;

use lazy_static::*;



#[no_mangle]
pub fn kthread_create(entry: usize) {

    println!("kthread_create");
    
    // create kernel thread
    let new_tcb = TaskControlBlock::new_kernel_thread(entry);

    // add kernel thread into TASK_MANAGER
    add_task(new_tcb.clone());
}

#[no_mangle]
pub fn kthread1(){
    println!("kernel thread {:?} STARTING", 1);
    for i in 0..10 {
        println!("kernel thread: {} counter: {}", 1, i);
    }
    println!("kernel thread {:?} FINISHED", 1);
    kthread_stop();
}

#[no_mangle]
pub fn kthread2(){
    println!("kernel thread {:?} STARTING", 2);
    for i in 0..10 {
        println!("kernel thread: {} counter: {}", 2, i);
    }
    println!("kernel thread {:?} FINISHED", 2);
    kthread_stop();
}

#[no_mangle]
pub fn kthread3(){
    println!("kernel thread {:?} STARTING", 3);
    for i in 0..10 {
        println!("kernel thread: {} counter: {}", 3, i);
    }
    println!("kernel thread {:?} FINISHED", 3);
    kthread_stop();
}

pub fn kthread_stop(){
    do_exit();
}
#[no_mangle]
pub fn do_exit(){
    println!("kthread do exit");
    exit_kthread_and_run_next(0);
    panic!("Unreachable in sys_exit!");
}

pub fn kthread_yield(){
    suspend_current_and_run_next();
}


#[no_mangle]
pub fn kernel_stackful_coroutine_test() {
    println!("kernel_stackful_coroutine_test");
    kthread_create(kthread1 as usize);
    kthread_create(kthread2 as usize);
    kthread_create(kthread3 as usize);
}



#[no_mangle]
pub fn exit_kthread_and_run_next(exit_code: i32) {
    println!("exit_kthread_and_run_next");

    let mut initproc_inner = INITPROC.inner_exclusive_access();
    let task = take_current_task().unwrap();


    // **** hold current PCB lock
    let wl = WAIT_LOCK.lock();

    let mut inner = task.inner_exclusive_access();

    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;

    // Record exit code
    inner.exit_code = Some(exit_code);

    //clean up children dealloc resources
    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    // deallocate fdtable
    inner.fd_table.clear();

    drop(inner);
    drop(task);
    drop(wl);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}


pub fn new_kthread_trap_cx(entry: usize, ksatck_top:usize) -> TrapContext {
    let mut cx: TrapContext = Default::default();
    let kernel_token = kernel_token();

    let mut sstatus = sstatus::read();
    
    sstatus.set_spp(SPP::Supervisor);
    // Supervisor Previous Interrupt Enable
    sstatus.set_spie(true);
    // Supervisor Interrupt Disable
    sstatus.set_sie(false);

    cx.sepc = entry;
    cx.kernel_satp = kernel_token;
    cx.trap_handler = trap_from_kernel as usize;
    cx.sstatus = sstatus;
    
    // for kthread only use one stack
    cx.x[2] = ksatck_top;
    cx.kernel_sp = ksatck_top;
    cx
}

use alloc::alloc::{alloc, dealloc, Layout};

#[derive(Clone)]
pub struct KStack(usize);

const STACK_SIZE: usize = 0x8000;

impl KStack {
    pub fn new() -> KStack {
        let bottom =
            unsafe {
                alloc(Layout::from_size_align(STACK_SIZE, STACK_SIZE).unwrap())
            } as usize;
        KStack(bottom)
    }

    pub fn top(&self) -> usize {
        self.0 + STACK_SIZE
    }
}
use core::fmt::{self, Debug, Formatter};
impl Debug for KStack {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("KStack:{:#x}", self.0))
    }
}

impl Drop for KStack {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.0 as _,
                Layout::from_size_align(STACK_SIZE, STACK_SIZE).unwrap()
            );
        }
    }
}



lazy_static! {
    static ref KERNEL_TGID_ALLOCATOR : Mutex<RecycleAllocator> = Mutex::new(RecycleAllocator::new(1));
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct TgidHandle(pub usize);

pub fn kernel_tgid_alloc() -> TgidHandle {
    TgidHandle(KERNEL_TGID_ALLOCATOR.lock().alloc())
}

// impl Drop for TgidHandle {
//     fn drop(&mut self) {
//         KERNEL_TGID_ALLOCATOR.lock().dealloc(self.0);
//     }
// }


pub fn kthread_trap_cx_bottom_from_tid(tgid: usize) -> usize {
    0x80801000 + tgid * PAGE_SIZE
}

pub fn kthread_stack_bottom_from_tid(tgid: usize) -> usize {
    0x80900000 + tgid * STACK_SIZE
}