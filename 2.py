import os

os.system("cd ../user && cargo build --release")
os.system("cd ../easy-fs-fuse && cargo run --release -- -s ../user/src/bin/ -t ../user/target/riscv64gc-unknown-none-elf/release/")
os.system("cargo build --features board_qemu")
