use {
    core::fmt,
    log::{self, Level, LevelFilter, Log, Metadata, Record},
};

use core::fmt::{Arguments, Result, Write};
use core::arch::asm;
use lock::Mutex;

use crate::drivers::chardev::{CharDevice, UART};

struct Uart;

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> Result {
        for c in s.bytes() {
            UART.write(c as u8);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! user_print {
    ($($arg:tt)*) => ({
        $crate::console::user_print(format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! user_println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::user_print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}

pub fn user_putfmt(fmt: Arguments) {
    Uart.write_fmt(fmt).unwrap();
}

#[allow(dead_code)]
pub fn user_print(args: fmt::Arguments) {
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






struct KernelConsole;

fn putchar(c: u8) {
    super::sbi::console_putchar(c as usize);
}

impl Write for KernelConsole {
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
    static CONSOLE: Mutex<KernelConsole> = Mutex::new(KernelConsole);
    CONSOLE.lock().write_fmt(fmt).unwrap();
}

pub fn init() {
    log::set_logger(&SimpleLogger).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("info") => LevelFilter::Info,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::kernel_print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}

/// print
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::console::kernel_print(format_args!($($arg)*));
    });
}

/// Add escape sequence to print with color in Linux console
macro_rules! with_color {
    ($args: ident, $color_code: ident) => {{
        format_args!("\u{1B}[{}m{}\u{1B}[0m", $color_code as u8, $args)
    }};
}

/// Add escape sequence to print with color in Linux console
macro_rules! hart_with_color {
    ($args: ident, $color_code: ident, $hart_id: ident) => {{
        format_args!("\u{1B}[{}m[hart {}] {}\u{1B}[0m", $color_code as u8, $hart_id, $args)
    }};
}

fn print_hart_with_color(args: Arguments, color_code: u8, hart_id: usize) {
    putfmt(hart_with_color!(args, color_code, hart_id));
}

fn print_in_color(args: fmt::Arguments, color_code: u8) {
    putfmt(with_color!(args, color_code));
}

#[allow(dead_code)]
pub fn kernel_print(args: fmt::Arguments) {
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

struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
            print_in_color(
                format_args!(
                    "[{:>5}][{},-] {}\n",
                    record.level(),
                    hart_id(),
                    record.args()
                ),
                level_to_color_code(record.level()),
            );
        }
    
    fn flush(&self) {}
}

fn level_to_color_code(level: Level) -> u8 {
    match level {
        Level::Error => 31, // Red
        Level::Warn => 34,  // BrightYellow
        Level::Info => 33,  // Blue
        Level::Debug => 32, // Green
        Level::Trace => 90, // BrightBlack
    }
}


pub fn hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_id);
    }
    hart_id
}
