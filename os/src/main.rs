#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(stdsimd)]
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

use core::arch::global_asm;
use core::arch::asm;

global_asm!(include_str!("entry.asm"));

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

use core::sync::atomic::{AtomicBool, Ordering};
use core::hint::spin_loop;
static AP_CAN_INIT: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub fn rust_main(hart_id: usize) -> ! {

    if hart_id == 0 {
        clear_bss();
        mm::init();
        println!("[kernel] Hello, world!");
        mm::remap_test();
        console::init();
        thread_local_init();
        trap::init();
        trap::enable_timer_interrupt();
        timer::set_next_trigger();
        fs::list_apps();
        task::kthread::kthreadd_create();
        task::kthread_test::kthread_test();
        task::add_initproc();

        
        AP_CAN_INIT.store(true, Ordering::Relaxed);
    }else {
        init_other_cpu();
    }
    
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
            println!("init done sp: {:#x}",  sp);
        }
    }
}

pub fn others_main(){
    mm::init_kernel_space();
    thread_local_init();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
}

pub fn thread_local_init() {
    unsafe { riscv::register::sstatus::set_sum(); }
}
/// Get current cpu id
pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}

