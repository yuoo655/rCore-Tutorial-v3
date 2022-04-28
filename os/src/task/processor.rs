use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{ProcessControlBlock, TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}

static mut idle_ptr: usize = 0;

impl Processor {
    pub fn new() -> Self {
        let idle_task_cx = TaskContext::zero_init();
        // unsafe {
        //     idle_ptr = &idle_task_cx as *const TaskContext  as usize;
        //     println!("idle ptr {:#x?}", idle_ptr);

        // }
        Self {
            current: None,
            idle_task_cx: idle_task_cx,
        }
    }
    pub fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }

}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

pub fn run_tasks() {
    // loop {
    //     let mut processor = PROCESSOR.exclusive_access();
        
    //     if let Some(task) = fetch_task() {

    //         let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();

    //         println!("acquire lock");
    //         // access coming task TCB exclusively
    //         let mut task_inner = task.inner_exclusive_access();

    //         let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;

    //         println!("set task state");
    //         task_inner.task_status = TaskStatus::Running;

    //         println!("release task inner");
    //         // release coming task TCB manually
    //         drop(task_inner);

    //         println!("set processor current task");
    //         processor.current = Some(task);

    //         println!("release processor");
    //         // release processor manually
    //         drop(processor);
            

    //         println!("switch");
    //         unsafe{
    //             // let idle_task_cx_ptr = idle_ptr as *mut TaskContext;
    //             println!(
    //                 "[schedule] next_task_cx_ptr: {:x?}, task cx: {:x?}",
    //                 next_task_cx_ptr,
    //                 unsafe { &*next_task_cx_ptr }
    //             );
    //             println!(
    //                 "[schedule] idle task cx ptr: {:x?}, task cx: {:x?}",
    //                 idle_task_cx_ptr,
    //                 unsafe { &*idle_task_cx_ptr }
    //             );
    //             __switch(idle_task_cx_ptr, next_task_cx_ptr);
    //         }
    //     } else {
    //         println!("no tasks available in run_tasks");
    //     }
    // }

    loop {
        let mut processor = PROCESSOR.exclusive_access();

        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task.clone());
            // release processor manually
            drop(processor);
            unsafe {
                __switch(
                    idle_task_cx_ptr,
                    next_task_cx_ptr,
                );
            }
        } else {
            println!("no tasks available in run_tasks");    
        }
    }
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn current_process() -> Arc<ProcessControlBlock> {
    current_task().unwrap().process.upgrade().unwrap()
}

pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

pub fn current_trap_cx_user_va() -> usize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .trap_cx_user_va()
}

pub fn current_kstack_top() -> usize {
    current_task().unwrap().kstack
}

pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe{
        // let idle_task_cx_ptr = idle_ptr as *mut TaskContext;
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }

}

