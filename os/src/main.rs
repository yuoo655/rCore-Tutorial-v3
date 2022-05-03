//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`task`]: Task management
//! - [`syscall`]: System call handling and implementation
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`task::run_first_task()`] and for the first time go to
//! userspace.

// #![deny(missing_docs)]
// #![deny(warnings)]
#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

use core::arch::global_asm;

#[cfg(feature = "board_k210")]
#[path = "boards/k210.rs"]
mod board;
#[cfg(not(any(feature = "board_k210")))]
#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
extern crate log;
#[macro_use]
mod console;
#[macro_use]
mod logging;

mod config;
mod lang_items;
mod loader;
mod sbi;
mod sync;
pub mod syscall;
pub mod task;
mod timer;
pub mod trap;
mod mm;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

use core::sync::atomic::{AtomicBool, Ordering};
use core::hint::{spin_loop};
use core::arch::asm;

extern crate alloc;

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

static AP_CAN_INIT: AtomicBool = AtomicBool::new(false);

/// the rust entry-point of os
#[no_mangle]
pub fn rust_main(hard_id : usize) -> ! {

    if hard_id == 0{
        clear_bss();
        unsafe{
            mm::init_heap_allocator();
        }
        logging::init();
        println!("hart [{:?}] [kernel] Hello, world!", hard_id);
        trap::init();
        trap::enable_timer_interrupt();
        timer::set_next_trigger();    
        loader::load_apps();
        task::add_user_tasks();

        // AP_CAN_INIT.store(true, Ordering::Relaxed);
    }else {
        init_other_cpu();
    }

    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

/// initialize the other cpu
pub fn init_other_cpu(){
    let hart_id = hart_id();
    if hart_id != 0 {
        // while !AP_CAN_INIT.load(Ordering::Relaxed) {
        //     // spin_loop();
        // }
        others_main();
        unsafe {
            let sp: usize;
            asm!("mv {}, sp", out(reg) sp);
            println!("hart[{:?}] init done sp:{:x?}", hart_id,  sp);
        }
    }
}

/// initialize the other cpu main procedure
pub fn others_main(){
    clear_bss();
    unsafe{
        mm::init_heap_allocator();
    }
    trap::init();
    trap::enable_timer_interrupt();
    println!("hard[{:?}] initializing", hart_id());
}


/// Get current cpu id
pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}
