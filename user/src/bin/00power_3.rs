#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

const LEN: usize = 100;

use core::arch::asm;

#[no_mangle]
fn main() -> i32 {
    let p = 3u64;
    let m = 998244353u64;
    let iter: usize = 200000;
    let mut s = [0u64; LEN];
    let mut cur = 0usize;
    s[cur] = 1;
    for i in 1..=iter {
        let next = if cur + 1 == LEN { 0 } else { cur + 1 };
        s[next] = s[cur] * p % m;
        cur = next;
        if i % 10000 == 0 {
            println!("[hart {}] power_3 [{}/{}]",hart_id(), i, iter);
        }
    }
    println!("[hart {}] {}^{} = {}(MOD {})",hart_id(), p, iter, s[cur], m);
    println!("[hart {}] Test power_3 OK!", hart_id());
    0
}

/// Get current cpu id
pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}
