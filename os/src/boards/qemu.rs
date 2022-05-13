pub const CLOCK_FREQ: usize = 12500000;

pub const MMIO: &[(usize, usize)] = &[
    (0x1000_0000, 0x1000),
    (0x1000_1000, 0x1000),
    (0xC00_0000, 0x40_0000),
];

pub type BlockDeviceImpl = crate::drivers::block::VirtIOBlock;
pub type CharDeviceImpl = crate::drivers::chardev::NS16550a<VIRT_UART>;

pub const VIRT_PLIC: usize = 0xC00_0000;
pub const VIRT_UART: usize = 0x1000_0000;

use crate::drivers::block::BLOCK_DEVICE;
use crate::drivers::chardev::{CharDevice, UART};
use crate::drivers::plic::{IntrTargetPriority, PLIC};
use crate::{hart_id, println};
use riscv::register::sie;

// pub fn device_init() {
//     let mut plic = unsafe { PLIC::new(VIRT_PLIC) };
    
//     let hart_id: usize = hart_id();
//     let supervisor = IntrTargetPriority::Supervisor;
//     let machine = IntrTargetPriority::Machine;
//     plic.set_threshold(hart_id, supervisor, 0);
//     plic.set_threshold(hart_id, machine, 1);
//     for intr_src_id in [1usize, 10] {
//         plic.enable(hart_id, supervisor, intr_src_id);
//         plic.set_priority(intr_src_id, 1);
//     }
//     unsafe {
//         sie::set_sext();
//     }
// }

// pub fn irq_handler() {
//     let hart_id: usize = hart_id();
//     let mut plic = unsafe { PLIC::new(VIRT_PLIC) };
//     let intr_src_id = plic.claim(hart_id, IntrTargetPriority::Supervisor);
//     match intr_src_id {
//         1 => BLOCK_DEVICE.handle_irq(),
//         10 => UART.handle_irq(),
//         _ => panic!("unsupported IRQ {}", intr_src_id),
//     }
//     plic.complete(hart_id, IntrTargetPriority::Supervisor, intr_src_id);
// }

pub fn irq_handler() {
    
    // which device interrupted?
    if let Some(irq) = plic_claim() {
        match irq {
            1 => BLOCK_DEVICE.handle_irq(),
            10 => UART.handle_irq(),
            _ => panic!("unsupported IRQ {}", irq),
        }
        // Tell the PLIC we've served the IRQ
        plic_complete(irq);
    }
}


use core::ptr;
// qemu puts platform-level interrupt controller (PLIC) here.
pub const PLIC_BASE: usize = 0x0c000000;

/// qemu puts UART registers here in physical memory.
pub const UART0:usize = 0x10000000;
pub const UART0_IRQ: u32 = 10;

/// virtio mmio interface
pub const VIRTIO0:usize = 0x10001000;
pub const VIRTIO0_IRQ: u32 = 1;


pub fn device_init(){
    plic_init();
    plic_init_hart();
}

pub fn plic_init() {
    write(PLIC_BASE + (UART0_IRQ * 4) as usize, 1);
    write(PLIC_BASE + (VIRTIO0_IRQ * 4) as usize, 1);
}


pub fn plic_init_hart() {

    let hart_id = hart_id();
    // println!("hart {} plic senable {:#x?}", hart_id, plic_senable(hart_id));
    // println!("hart {} plic spriority {:#x?}", hart_id, plic_spriority(hart_id));

    // Set UART's enable bit for this hart's S-mode. 
    write(plic_senable(hart_id), (1 << UART0_IRQ) | (1 << VIRTIO0_IRQ));

    // Set this hart's S-mode pirority threshold to 0. 
    write(plic_spriority(hart_id), 0);

    unsafe {
        sie::set_sext();
    }
}

fn plic_senable(hart_id: usize) -> usize {
    PLIC_BASE + 0x2080 + hart_id * 0x100
}

fn plic_spriority(hart_id: usize) -> usize {
    PLIC_BASE + 0x201000 + hart_id * 0x2000
}

fn plic_sclaim(hart_id: usize) -> usize {
    PLIC_BASE + 0x201004 + hart_id * 0x2000
}

/// Ask the PLIC what interrupt we should serve. 
pub fn plic_claim() -> Option<u32> {
    let hart_id = hart_id();
    let interrupt = read(plic_sclaim(hart_id));
    if interrupt == 0 {
        None
    } else {
        Some(interrupt)
    }
}


/// Tell the PLIC we've served the IRQ
pub fn plic_complete(interrupt: u32) {
    let hart_id = hart_id();
    write(plic_sclaim(hart_id), interrupt);
}


fn write(addr: usize, val: u32) {
    unsafe {
        ptr::write(addr as *mut u32, val);
    }
}

fn read(addr: usize) -> u32 {
    unsafe {
        ptr::read(addr as *const u32)
    }
}