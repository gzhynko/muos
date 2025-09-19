use core::arch::{asm, naked_asm};
use cortex_m::interrupt;
use cortex_m::peripheral::SCB;
use cortex_m_rt::exception;
use crate::asm::{do_context_switch, do_setup};
use cortex_m_rt::ExceptionFrame;
use defmt;
use crate::scheduler::{schedule, with_scheduler};

use crate::{scheduler, thread};
use crate::memory::mpu_program_thread;
use crate::stack::STACK_SIZE;
use crate::thread::ThreadContext;

#[exception]
fn SysTick() {
    with_scheduler(|sched| sched.systick());
    cortex_m::peripheral::SCB::set_pendsv();
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    let scb = &*SCB::ptr();

    let hfsr = scb.hfsr.read();
    let cfsr = scb.cfsr.read();       // low byte = MemManage, next = BusFault, next = UsageFault
    let mmfar_valid = (cfsr & (1 << 7)) != 0;       // MemManage Fault Address Valid
    let mmfar       = scb.mmfar.read();
    let bfar_valid  = (cfsr & (1 << 15)) != 0;      // BusFault Address Valid
    let bfar        = scb.bfar.read();

    defmt::error!("🔴 HardFault! \
    \n  R0 = {:#010X}\
    \n  R1 = {:#010X}\
    \n  R2 = {:#010X}\
    \n  R3 = {:#010X}\
    \n  R12 = {:#010X}\
    \n  PC = {:#010X}\
    \n  LR = {:#010X}\
    \n  xPSR = {:#010X}\
    \n  HFSR = {:#010X}\
    \n  CFSR = {:#010X}\
    \n  MMFARVALID={} @ {:#010X}\
    \n  BFARVALID={} @ {:#010X}",
    ef.r0(), ef.r1(), ef.r2(), ef.r3(), ef.r12(), ef.pc(), ef.lr(), ef.xpsr(), hfsr, cfsr, mmfar_valid, mmfar, bfar_valid,  bfar);

    loop { }
}

#[exception]
unsafe fn MemoryManagement() -> ! {
    let scb = &*SCB::PTR;

    // The CFSR (Configurable Fault Status Register) lives at offset 0xC;
    // its low byte is the MemManage Status Bits.
    let cfsr = scb.cfsr.read();

    // Bit 7 of CFSR is MMARVALID: indicates MMFAR holds a valid address
    let mmar_valid = (cfsr & (1 << 7)) != 0;

    // Read the MMFAR (Memory Management Fault Address Register)
    let mmar = scb.mmfar.read();

    defmt::error!(
      "MemManage Fault!: {:#x} {}", mmar, mmar_valid
    );
    loop { /* lock up or reset the thread */ }
}

#[inline(always)]
pub unsafe fn handle_pend_sv() {
    defmt::trace!("handle_pend_sv");
    let exc_return = 0xFFFF_FFFD;

    let maybe_ptrs: Option<(*mut ThreadContext, *mut ThreadContext)> =
        with_scheduler(|sched|
            sched.schedule()
        );

    // 3) …then do the actual switch *after* we've dropped the lock
    if let Some((prev_ptr, next_ptr)) = maybe_ptrs {
        defmt::trace!("run do_context_switch with following: prev: {:#x} next: {:#x}", (*prev_ptr).stack_addr, (*next_ptr).stack_addr);

        // setup MPU for the new thread
        let (stack_base, stack_size) = with_scheduler(|sched|
            sched.get_current_thread_stack()
        );
        mpu_program_thread(stack_base, stack_size);

        do_context_switch(prev_ptr, next_ptr, exc_return);
    }

    defmt::trace!("handle_pend_sv RUNOFF!");
}

#[naked]
#[no_mangle]
pub unsafe extern "C" fn PendSV() -> ! {
    naked_asm!(
    "bl     {handler}",
    "ldr    lr, ={exc_ret}",
    "bx     lr",
    handler = sym handle_pend_sv,
    exc_ret = const 0xFFFF_FFFDu32,
    )
}
