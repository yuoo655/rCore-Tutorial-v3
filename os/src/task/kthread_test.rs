use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::sync::ksync::{
    k_mutex_create,
    k_mutex_lock,
    k_mutex_unlock,
};

use super::{
    current_task,
};

use crate::task::kthread::{
    do_exit,
    create_kthread,
    kthread_should_stop,
    send_kthread_stop, wait_for_completion
};
use crate::timer::get_time_ms;

#[no_mangle]
pub fn kthread_test() {
    println!("kthread_test");

    // pid 0 = kthreadd
    // pid 1
    create_kthread(kthread_print as usize);
    // pid 2
    create_kthread(kthread_get_current as usize);
    // pid 3
    create_kthread(kthread_runs_until_rec_kthread_stop as usize);
    // pid 4
    create_kthread(kthread_stop_test as usize);
    // pid 5
    create_kthread(kthread_test_sem as usize);
}

#[no_mangle]
pub fn kthread_print(){
    let pid = current_task().unwrap().pid.0;
    println!("kthread pid {:?} STARTING", pid);
    for i in 0..10 {
        println!("kthread pid {:?} counter: {}", pid, i);
    }
    println!("kthread pid {:?} FINISHED", pid);
    do_exit(0);
}


#[no_mangle]
pub fn kthread_get_current(){
    // loop {
        if let Some(tcb) = current_task() {
            println!("get current kthread pid: {}", tcb.pid.0);

            let inner = tcb.inner_exclusive_access();

            println!("kthread get current task inner lock");

            drop(inner);
        }
    // }
    do_exit(0);
}


#[no_mangle]
pub fn kthread_runs_until_rec_kthread_stop(){
    let pid = current_task().unwrap().pid.0;
    while !kthread_should_stop(){
        println!("kthread pid {} waiting for kthread_stop", pid);
    }   
    println!("kthread pid {} received kthread_stop do_exit now", pid);

    do_exit(0);
}


pub fn kthread_stop_test(){
    let time_start = get_time_ms();

    while get_time_ms() - time_start < 1000 {
    }
    println!("sending kthread_stop to kthread {}", 3);
    send_kthread_stop(3);
}

static mut A: usize = 0;
const PER_THREAD: usize = 1000;
const THREAD_COUNT: usize = 16;

unsafe fn f(){
    let mut t = 2usize;
    for _ in 0..PER_THREAD {
        k_mutex_lock(0);
        let a = &mut A as *mut usize;
        let cur = a.read_volatile();
        for _ in 0..500 {
            t = t * t % 10007;
        }
        a.write_volatile(cur + 1);

        k_mutex_unlock(0);
    }
    do_exit(t as i32)
}

#[no_mangle]
pub fn kthread_test_sem(){
    let start = get_time_ms();

    let x = k_mutex_create(false);

    let mut v = Vec::new();

    for _ in 0..THREAD_COUNT {
        v.push( create_kthread(f as usize) );
    }

    println!("create {} kthreads", THREAD_COUNT);

    for pid in v.iter() {
        wait_for_completion(*pid as usize);
    }

    assert_eq!(unsafe { A }, PER_THREAD * THREAD_COUNT);
    println!("kthread sem test pass time cost is {}ms", get_time_ms() - start);
    do_exit(0);
}