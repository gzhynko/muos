#![feature(naked_functions, asm)]
#![no_std]

mod asm;
pub mod numbers;

use core::arch::{asm, naked_asm};
use crate::asm::{syscall0, syscall1};
use crate::numbers::MAX_SYSCALL_ID;


/// Signature for a syscall handler.
pub type SyscallFn = unsafe extern "C" fn(usize, usize, usize);

/// The central dispatch table.
static mut HANDLERS: [Option<SyscallFn>; MAX_SYSCALL_ID] = [None; MAX_SYSCALL_ID];

/// Register a handler for `id`.  Call this *before* any svc occurs.
pub fn register(id: usize, func: SyscallFn) {
    assert!(id < MAX_SYSCALL_ID);
    unsafe { HANDLERS[id] = Some(func) }
}

/// Look up the handler for `id`.
pub fn get(id: usize) -> Option<SyscallFn> {
    if id < MAX_SYSCALL_ID {
        unsafe { HANDLERS[id] }
    } else {
        None
    }
}

#[inline(always)]
pub fn scheduler_boot() {
    syscall0(numbers::SCHEDULER_BOOT)
}

#[inline(always)]
pub fn yield_now() {
    syscall0(numbers::YIELD_NOW)
}

#[inline(always)]
pub fn sleep_ms(ms: u32) {
    syscall1(numbers::SLEEP_MS, ms as usize)
}

#[inline(always)]
pub fn exit_thread() {
    syscall0(numbers::EXIT_THREAD)
}

/// Naked SVC entrypoint.  Reads the mailbox and jumps to `syscall_dispatcher`.
#[naked]
#[no_mangle]
pub unsafe extern "C" fn SVCall() -> ! {
    naked_asm!(
    // tail‑call into dispatcher(id,r1,r2,r3)
    "b {disp}",
    disp = sym syscall_dispatcher,
    )
}

/// Dispatches syscalls.  Looks up the handler and calls it.
#[no_mangle]
pub unsafe extern "C" fn syscall_dispatcher(
    id: usize,    // r0
    a1: usize,    // r1
    a2: usize,    // r2
    a3: usize,    // r3
) {
    //defmt::trace!("syscall dispatch: {:#x} {:#x} {:#x} {:#x}", id, a1, a2, a3);
    if let Some(f) = get(id) {
        f(a1, a2, a3)
    } else {
        panic!("syscall_dispatcher: no handler registered for id {}", id)
    }
}
