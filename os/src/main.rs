//! The main module and entrypoint
//!
//! The operating system and app also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality [`clear_bss()`]. (See its source code for
//! details.)
//!
//! We then call [`println!`] to display `Hello, world!`.

#![no_std]
#![no_main]
#![feature(panic_info_message)]

use core::arch::global_asm;

#[macro_use]
extern crate log;
#[macro_use]
mod console;
#[macro_use]
mod logging;

mod lang_items;
mod sbi;

global_asm!(include_str!("entry.asm"));

/// clear BSS segment
pub fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}


use core::sync::atomic::{AtomicBool, Ordering};
use core::hint::{spin_loop, self};
use core::arch::asm;

static AP_CAN_INIT: AtomicBool = AtomicBool::new(false);

/// the rust entry-point of os
#[no_mangle]
pub fn rust_main(hart_id: usize) -> ! {

    if hart_id == 0 {
        extern "C" {
            fn stext();               // begin addr of text segment
            fn etext();               // end addr of text segment
            fn srodata();             // start addr of Read-Only data segment
            fn erodata();             // end addr of Read-Only data ssegment
            fn sdata();               // start addr of data segment
            fn edata();               // end addr of data segment
            fn sbss();                // start addr of BSS segment
            fn ebss();                // end addr of BSS segment
            fn boot_stack();          // stack bottom
            fn boot_stack_top();      // stack top
        }
        clear_bss();
        logging::init();
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(
            "boot_stack [{:#x}, {:#x})",
            boot_stack as usize, boot_stack_top as usize
        );
        println!(".bss [{:#x}, {:#x})", sbss as usize, ebss as usize);
        
        println!("hart[{:?}] Hello, world!", hart_id);

        unsafe {
            let sp: usize;
            asm!("mv {}, sp", out(reg) sp);
            println!("hart[{:?}] init done sp:{:x?}", hart_id,  sp);
        }
        send_ipi();
        AP_CAN_INIT.store(true, Ordering::Relaxed);

    }else {
        init_other_cpu();
    }

    loop {
        spin_loop();
    }
    // panic!("Shutdown machine!");
}

pub fn init_other_cpu(){
    let hart_id = hart_id();
    if hart_id != 0 {
        while !AP_CAN_INIT.load(Ordering::Relaxed) {
            hint::spin_loop();
        }
        others_main();
        unsafe {
            let sp: usize;
            asm!("mv {}, sp", out(reg) sp);
            println!("hart[{:?}] init done sp:{:x?}", hart_id,  sp);
            println!("hart[{:?}] Hello, world!", hart_id);
        }
    }
}

pub fn others_main(){
    clear_bss();
    println!("hard[{:?}] initializing (clear bss)", hart_id());
}

pub fn send_ipi(){
    //rCore-tutorial目前的rustsbi版本为0.2.0-alpha.6 所有核一次启动了,不需要再发送ipi

    // let hart_id = hart_id();
    // for i in 1..4 {
    //     debug!("[hart {}] Start hart[{}]", hart_id, i);
    //     let mask: usize = 1 << i;
    //     sbi::send_ipi(&mask as *const _ as usize);
    // }
}
pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}