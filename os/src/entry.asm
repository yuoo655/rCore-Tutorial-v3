    .section .text.entry
    .globl _start
_start:
    mv tp, a0
    la sp, boot_stack
    addi t0, a0, 1
    slli t0, t0, 16
    add sp, sp, t0

    call rust_main

    .section .bss.stack
    .globl boot_stack
boot_stack:
    .space 4096 * 16 * 4
    .globl boot_stack_top
boot_stack_top: