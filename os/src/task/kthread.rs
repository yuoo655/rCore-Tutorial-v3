use riscv::register::sstatus::{self, SPP};
use crate::task::pid2task;
use crate::trap::{
    TrapContext, trap_from_kernel
};
use crate::mm::kernel_token;
use crate::config::{
    MEMORY_END, PAGE_SIZE,
};
use super::{
    TaskControlBlock,
    add_task_first_time,
    suspend_current_and_run_next,
    schedule,
    RecycleAllocator,
    take_current_task,
    TaskStatus,
    WAIT_LOCK,
    sleep_task,
    remove_from_pid2task,
    current_task
};
use lock::Mutex;

use lazy_static::*;


#[no_mangle]
pub fn kthreadd_create(){
    // in order to support mutex/semaphore and other synchronization mechanism
    // kthread should have a parent kthread. other kthread should be forked from this kthread    
    
    // create kernel thread
    let new_tcb = TaskControlBlock::kthreadd_create(kthreadd as usize);

    // add kernel thread into TASK_MANAGER
    add_task_first_time(new_tcb.clone());

    println!("[kernel] kthreadd created");
}

pub fn kthreadd() -> !{

    println!("kthreadd started");
    // kthreadd is to manage and schedule other kernel threads
    // other kthread will be forked from this kthread
    // kthreadd will never stop
    // kthreadd is only used to create other kthreads for now 
    
    
    
    // if let Some(kthread_entry) = kthread_create_list.lock().pop() {
    //     let tcb = create_kthread(kthread_entry);
    //     add_task_first_time(tcb);
    // }else{    
    //     schedule();
    // }
            
    loop{
    }
}


#[no_mangle]
pub fn create_kthread(entry: usize) -> i32 {
    // get kthreadd
    let kthreadd = pid2task(0).unwrap();
    
    // create kernel thread (copy all things like fork)
    let new_tcb = kthreadd.new_kernel_thread(entry, 0);
    
    let new_pid = new_tcb.pid.0;

    // add kernel thread into TASK_MANAGER
    add_task_first_time(new_tcb.clone());
    
    new_pid as i32
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

    // sepc = ?
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
    //guard page
    let memory_end = MEMORY_END + PAGE_SIZE;
    memory_end + tgid * PAGE_SIZE
}

pub fn kthread_stack_bottom_from_tid(tgid: usize) -> usize {
    //guard page +  trap cx for kthread
    let memory_end = MEMORY_END + PAGE_SIZE + 0x100000;
    memory_end + tgid * STACK_SIZE
}



pub fn kthread_yield(){
    suspend_current_and_run_next();
}


#[no_mangle]
pub fn do_exit(exit_code: i32){
    exit_kthread_and_run_next(0);
    panic!("Unreachable in sys_exit!");
}


#[no_mangle]
pub fn exit_kthread_and_run_next(exit_code: i32) {
    // println!("exit_kthread_and_run_next");

    let task = take_current_task().unwrap();

    // **** hold current PCB lock
    let wl = WAIT_LOCK.lock();
    let mut inner = task.inner_exclusive_access();
    let task_cx_ptr = inner.get_task_cx_ptr();

    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = Some(exit_code);

    let pid = task.pid.0;
    let tgid = task.tgid;

    // normal kthread exit
    if pid == tgid {

        // clean up children dealloc resources
        inner.children.clear();
        

        remove_from_pid2task(pid);

        // todo recycle mmaped memory
    }

    drop(inner);
    sleep_task(task.clone());
    drop(wl);

    
    schedule(task_cx_ptr);
}


pub fn kthread_should_stop() -> bool{
    let flags = current_task().unwrap().inner_exclusive_access().flags;
    flags == KthreadBits::KthreadShouldStop as usize
}


#[derive(Copy, Clone, PartialEq)]
pub enum KthreadBits {
    KthreadShouldStop = 1,
    KthreadShouldPark = 1 << 1,    
}

pub fn send_kthread_stop(pid:usize){
    let task = pid2task(pid).unwrap();

    let mut inner = task.inner_exclusive_access();

    inner.flags |= KthreadBits::KthreadShouldStop as usize;

    drop(inner);

    do_exit(0);
}


pub fn wait_for_completion(pid:usize){

    while pid2task(pid).is_some() {
        // kthread_yield();
    }    
    return;
}



