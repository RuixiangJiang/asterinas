//! The framework part of KxOS.
#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]
#![feature(negative_impls)]
#![feature(fn_traits)]
#![feature(const_maybe_uninit_zeroed)]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]
#![feature(new_uninit)]
#![feature(link_llvm_intrinsics)]

extern crate alloc;

pub(crate) mod cell;
pub mod config;
pub mod cpu;
pub mod device;
mod error;
pub mod log;
pub(crate) mod mm;
pub mod prelude;
pub mod sync;
pub mod task;
pub mod timer;
pub mod trap;
pub mod user;
mod util;
pub mod vm;
pub(crate) mod x86_64_util;

use core::{mem, panic::PanicInfo};

pub use self::error::Error;
pub use self::prelude::Result;
pub(crate) use self::sync::up::UPSafeCell;

use alloc::vec::Vec;
use bootloader::{
    boot_info::{FrameBuffer, MemoryRegionKind},
    BootInfo,
};
pub use mm::address::{align_down, align_up, is_aligned, virt_to_phys};
pub use trap::{allocate_irq, IrqAllocateHandle, TrapFrame};
use trap::{IrqCallbackHandle, IrqLine};
pub use util::AlignExt;
pub use vm::Pod;
use x86_64_util::enable_common_cpu_features;

static mut IRQ_CALLBACK_LIST: Vec<IrqCallbackHandle> = Vec::new();

#[cfg(not(feature = "serial_print"))]
pub use crate::screen_print as print;
#[cfg(not(feature = "serial_print"))]
pub use crate::screen_println as println;

#[cfg(feature = "serial_print")]
pub use crate::serial_print as print;
#[cfg(feature = "serial_print")]
pub use crate::serial_println as println;

pub fn init(boot_info: &'static mut BootInfo) {
    let siz = boot_info.framebuffer.as_ref().unwrap() as *const FrameBuffer as usize;
    let mut memory_init = false;
    // memory
    for region in boot_info.memory_regions.iter() {
        if region.kind == MemoryRegionKind::Usable {
            let start: u64 = region.start;
            let size: u64 = region.end - region.start;
            println!(
                "[kernel] physical frames start = {:x}, size = {:x}",
                start, size
            );
            mm::init(start, size);
            memory_init = true;
            break;
        }
    }
    if !memory_init {
        panic!("memory init failed");
    }
    device::init(boot_info.framebuffer.as_mut().unwrap());
    device::framebuffer::WRITER.lock().as_mut().unwrap().clear();
    trap::init();
    enable_common_cpu_features();
    unsafe {
        for i in 0..256 {
            IRQ_CALLBACK_LIST.push(IrqLine::acquire(i as u8).on_active(general_handler))
        }
    }
    // uncomment below code to enable timer interrupt
    // x86_64_util::enable_interrupts_and_hlt();
}
fn general_handler(trap_frame: &TrapFrame) {
    println!("{:#x?}", trap_frame);
    println!("rip = 0x{:x}", trap_frame.rip);
    println!("rsp = 0x{:x}", trap_frame.rsp);
    println!("cr2 = 0x{:x}", trap_frame.cr2);
    // println!("rbx = 0x{:x}", trap_frame.)
    panic!("couldn't handler trap right now");
}

#[inline(always)]
pub(crate) const fn zero<T>() -> T {
    unsafe { mem::MaybeUninit::zeroed().assume_init() }
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\n", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]");
    serial_println!("Error: {}", info);
    exit_qemu(QemuExitCode::Failed);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
    unreachable!()
}