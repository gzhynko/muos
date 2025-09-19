use core::arch::{asm, naked_asm};
use crate::thread::{Thread, ThreadContext};

#[naked]
#[no_mangle]
pub unsafe extern "C" fn do_context_switch(
    prev_ctx: *mut ThreadContext,   // r0
    next_ctx: *mut ThreadContext,   // r1
    lr:       u32,                  // r2 = the EXC_RETURN
) {
    naked_asm!(
    // 1) grab the *old* PSP, push r4–r11, write it back
    "    mrs    r3, PSP",         // r3 = old SP
    "    stmdb  r3!, {{r4-r11}}", // push callee-saved
    "    str    r3, [r0]",        // prev_ctx->stack_addr = new SP

    // 2) load the *new* thread’s SP (regs_start), pop its r4–r11, restore PSP
    "    ldr    r3, [r1]",        // r3 = next_ctx->stack_addr
    "    ldmia  r3!, {{r4-r11}}", // pop callee-saved
    "    msr    PSP, r3",         // PSP = the frame_start

    // 3) make sure memory is coherent before EXC_RETURN
    "    dsb",
    "    isb",

    // 4) exit the exception into thread-mode
    "    mov    lr, r2",          // LR = EXC_RETURN
    "    bx     lr",              // -> pops the HW exception-frame on PSP
    )
}

#[naked]
#[no_mangle]
pub unsafe extern "C" fn do_setup(
    psp:       u32,  // → r0
    control:   u32,  // → r1
    exc_return: u32, // → r2
) -> ! {
    naked_asm!(
    // 1) set up our process‑stack pointer
    "msr   PSP,  r0",

    // 2) switch CONTROL (privilege/stack)
    "msr   CONTROL, r1",
    "isb",

    // 3) prepare EXC_RETURN in LR
    "mov   lr,   r2",

    // 4) exception‑return into the new thread
    "bx    lr",
    )
}
