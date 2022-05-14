//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`task`]: Task management
//! - [`syscall`]: System call handling and implementation
//! - [`mm`]: Address map using SV39
//! - [`sync`]:Wrap a static data structure inside it so that we are able to access it without any `unsafe`.
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`task::run_tasks()`] and for the first time go to
//! userspace.

// #![deny(missing_docs)]
// #![deny(warnings)]

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
mod console;
mod config;
mod lang_items;
mod loader;
pub mod mm;
mod sbi;
pub mod sync;
pub mod syscall;
pub mod task;
mod timer;
pub mod trap;

use core::arch::global_asm;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));
/// clear BSS segment
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

use core::arch::asm;
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
        task::add_initproc();
        println!("after initproc!");
        trap::init();
        trap::enable_timer_interrupt();
        timer::set_next_trigger();
        loader::list_apps();
        
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

// #[no_mangle]
// /// the rust entry-point of os
// pub fn rust_main() -> ! {
//     clear_bss();
//     println!("[kernel] Hello, world!");
//     mm::init();
//     mm::remap_test();
//     task::add_initproc();
//     println!("after initproc!");
//     trap::init();
//     //trap::enable_interrupt();
//     trap::enable_timer_interrupt();
//     timer::set_next_trigger();
//     loader::list_apps();
//     task::run_tasks();
//     panic!("Unreachable in rust_main!");
// }
