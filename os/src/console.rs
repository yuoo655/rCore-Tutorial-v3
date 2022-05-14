use {
    core::fmt,
};

use core::fmt::{Arguments, Result, Write};
use core::arch::asm;
use spin::Mutex;

struct Console;

fn putchar(c: u8) {
    super::sbi::console_putchar(c as usize);
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> Result {
        for c in s.bytes() {
            if c == 127 {
                putchar(8);
                putchar(b' ');
                putchar(8);
            } else {
                putchar(c);
            }
        }
        Ok(())
    }
}

pub fn putfmt(fmt: Arguments) {
    static CONSOLE: Mutex<Console> = Mutex::new(Console);
    CONSOLE.lock().write_fmt(fmt).unwrap();
}

/// print
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::console::print(format_args!($($arg)*));
    });
}

///println
#[macro_export]
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

/// Add escape sequence to print with color in Linux console
macro_rules! with_color {
    ($args: ident, $color_code: ident) => {{
        format_args!("\u{1B}[{}m{}\u{1B}[0m", $color_code as u8, $args)
    }};
}

// /// Add escape sequence to print with color in Linux console
// macro_rules! hart_with_color {
//     ($args: ident, $color_code: ident, $hart_id: ident) => {{
//         format_args!("\u{1B}[{}m[hart {}] {}\u{1B}[0m", $color_code as u8, $hart_id, $args)
//     }};
// }

// fn print_hart_with_color(args: Arguments, color_code: u8, hart_id: usize) {
//     putfmt(hart_with_color!(args, color_code, hart_id));
// }

fn print_in_color(args: fmt::Arguments, color_code: u8) {
    putfmt(with_color!(args, color_code));
}

#[allow(dead_code)]
pub fn print(args: fmt::Arguments) {
    let hart_id = hart_id();
    let color = match hart_id {
        0  => 96,
        1  => 94,
        2  => 95,
        3  => 93,
        _  => 97,
    };
    // print_hart_with_color(args, color as u8, hart_id);
    print_in_color(args, color as u8);
}

pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}