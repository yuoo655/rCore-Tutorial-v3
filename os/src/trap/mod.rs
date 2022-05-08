mod context;

use crate::config::{TRAMPOLINE, TRAP_CONTEXT};
use crate::mm::kernel_token;
use crate::syscall::syscall;
use crate::task::{
    check_signals_error_of_current, current_add_signal, current_trap_cx, current_user_token,
    exit_current_and_run_next, suspend_current_and_run_next, SignalFlags, handle_signals,
    current_trap_cx_user_va,
};
use crate::timer::{check_timer, set_next_trigger};
use core::arch::{asm, global_asm};
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,sstatus, sscratch, sepc,
};

global_asm!(include_str!("trap.S"));

pub fn init() {
    set_kernel_trap_entry();
}

fn set_kernel_trap_entry() {
    extern "C" {
        fn __alltraps();
        fn __alltraps_k(); 
    }

    // 0xFFFF...F000
    let __alltraps_k_va = __alltraps_k as usize - __alltraps as usize + TRAMPOLINE;
    unsafe {
        stvec::write(__alltraps_k_va, TrapMode::Direct);
        sscratch::write(trap_from_kernel as usize);
        sstatus::set_sie();
    }
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[no_mangle]
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // jump to next instruction anyway
            let mut cx = current_trap_cx();
            cx.sepc += 4;        
            // get system call return value
            unsafe{
                sstatus::set_spie();
            }
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]);
            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            /*
            println!(
                "[kernel] {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            */
            current_add_signal(SignalFlags::SIGSEGV);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            current_add_signal(SignalFlags::SIGILL);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            check_timer();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    // handle signals (handle the sent signal)
    // handle_signals();

    // check error signals (if error then exit)
    if let Some((errno, msg)) = check_signals_error_of_current() {
        println!("[kernel] {}", msg);
        exit_current_and_run_next(errno);
    }
    trap_return();
}

#[no_mangle]
pub fn trap_return() -> ! {
    unsafe{
        sstatus::clear_sie();
    }
    set_user_trap_entry();
    let trap_cx_ptr = current_trap_cx_user_va();
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;

    unsafe {
        sstatus::set_spie();
        sstatus::set_spp(sstatus::SPP::User);
        asm!(
            "fence.i",
            "jr {restore_va}",
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,
            in("a1") user_satp,
            options(noreturn)
        );
    }
}


pub use context::TrapContext;
#[no_mangle]
pub fn trap_from_kernel(_trap_cx: &TrapContext) {
    let scause = scause::read();
    let stval = stval::read();

    let local_sstatus = sstatus::read();
    if local_sstatus.spp() != sstatus::SPP::Supervisor{
        panic!("trap_from_kernel(): not from supervisor mode");
    }
    if local_sstatus.sie()  {
        panic!("trap_from_kernel(): interrupts enabled");
    }
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            // crate::board::irq_handler();
        },
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            // check_timer();
        },
        _ => {
            panic!(
                "Unsupported trap from kernel: {:?}, stval = {:#x} memoryend:{:#x}!",
                scause.cause(),
                stval,
                crate::config::MEMORY_END
            );
        },
    }
}


#[no_mangle]
pub fn kernel_return() -> ! {
    loop{}
}

pub static mut TICKS: usize = 0;