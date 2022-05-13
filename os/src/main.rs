#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[cfg(feature = "board_k210")]
#[path = "boards/k210.rs"]
mod board;
#[cfg(not(any(feature = "board_k210")))]
#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
extern crate log;
#[macro_use]
pub mod console;


mod config;
mod drivers;
mod fs;
mod lang_items;
mod mm;
mod sbi;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;


core::arch::global_asm!(include_str!("entry.asm"));

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

use lazy_static::*;
use core::arch::asm;
use sync::UPIntrFreeCell;
use lock::Mutex;
use core::sync::atomic::{AtomicBool, Ordering, AtomicUsize};
use core::hint::spin_loop;
static AP_CAN_INIT: AtomicBool = AtomicBool::new(false);
use riscv::register::sie;



lazy_static::lazy_static! {
    static ref BOOTED_CPU_NUM: AtomicUsize = AtomicUsize::new(0);
}

lazy_static! {
    pub static ref DEV_NON_BLOCKING_ACCESS: lock::Mutex<bool> = lock::Mutex::new(false);
}


#[no_mangle]
pub fn rust_main(hart_id: usize, device_tree_paddr: usize) -> ! {

    if hart_id == 0 {
        clear_bss();
        mm::init();
        thread_local_init();
        board::device_init();
        fs::list_apps();
        trap::init();
        task::add_initproc();
        trap::enable_timer_interrupt();
        timer::set_next_trigger();
    
        BOOTED_CPU_NUM.fetch_add(1, Ordering::Release);
        AP_CAN_INIT.store(true, Ordering::Relaxed);
    }else{
        init_other_cpu();
    }
    
    
    wait_all_cpu_started();
    *DEV_NON_BLOCKING_ACCESS.lock() = true;
    
    
    println!("Hello");
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

pub fn init_other_cpu(){
    let hart_id = hart_id();
    if hart_id != 0 {
        while !AP_CAN_INIT.load(Ordering::Relaxed) {
            spin_loop();
        }
        others_main();
        unsafe {
            let sp: usize;
            asm!("mv {}, sp", out(reg) sp);
            println!("init done sp: {:#x}", sp);
        }
    }
}

pub fn others_main(){
    mm::init_kernel_space();
    thread_local_init();
    board::device_init();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    BOOTED_CPU_NUM.fetch_add(1, Ordering::Release);
}

/// Get current cpu id
pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}

pub fn thread_local_init() {
    // 允许内核读写用户态内存
    // 取决于 CPU 的 RISC-V 规范版本就行处理
    unsafe { riscv::register::sstatus::set_sum(); }
}

fn wait_all_cpu_started() {
    while BOOTED_CPU_NUM.load(Ordering::Acquire) < crate::config::CPU_NUM {
        spin_loop();
    }
}