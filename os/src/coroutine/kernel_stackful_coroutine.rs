use crate::task::{
    TaskControlBlock
};
use crate::task::add_task;
use alloc::sync::Arc;
use crate::trap::TrapContext;
use crate::task::PROCESSOR;


use crate::task::{
    exit_kthread_and_run_next,
    suspend_current_and_run_next,
};

#[no_mangle]
pub fn kthread_create(f: fn()) {

    println!("kthread_create");
    
    //创建内核线程
    let new_tcb = TaskControlBlock::create_kthread(f);
    let kernel_stack = new_tcb.get_kernel_stack();
    let new_task = Arc::new(new_tcb);

    //往调度器加任务,与用户线程放在一起调度.
    // println!("add task");
    add_task(Arc::clone(&new_task));
}


#[no_mangle]
pub fn kernel_stackful_coroutine_test() {
    println!("kernel_stackful_coroutine_test");
    kthread_create( ||
        {
            let id = 1;
            println!("kernel thread {:?} STARTING", id);
            for i in 0..10 {
                println!("kernel thread: {} counter: {}", id, i);
            }
            println!("kernel thread {:?} FINISHED", id);
            kthread_stop();
        }
    );
    kthread_create( ||
        {
            let id = 2;
            println!("kernel thread {:?} STARTING", 2);
            for i in 0..10 {
                println!("kernel thread: {} counter: {}", 2, i);
                kthread_yield();
            }
            println!("kernel thread {:?} FINISHED", 2);
            kthread_stop();
        }
    );
    kthread_create( ||
        {
            let id = 3;
            println!("kernel thread {:?} STARTING", 3);
            for i in 0..10 {
                println!("kernel thread: {} counter: {}", 3, i);
                kthread_yield();
            }
            println!("kernel thread {:?} FINISHED", 3);
            kthread_stop();
        }
    );
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
