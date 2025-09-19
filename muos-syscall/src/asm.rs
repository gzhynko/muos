use core::arch::asm;

/// Fire an SVC with no arguments.
#[inline(always)]
pub fn syscall0(id: usize) {
    unsafe {
        asm!(
        "svc 0",
        in("r0") id,
        options(nostack)
        );
    }
}

/// Fire an SVC with 1 argument in `r1`.
#[inline(always)]
pub fn syscall1(id: usize, a0: usize) {
    unsafe {
        asm!(
        "svc 0",
        in("r0") id,
        in("r1") a0,
        options(nostack)
        );
    }
}

/// Fire an SVC with 2 arguments in `r1`/`r2`.
#[inline(always)]
pub fn syscall2(id: usize, a0: usize, a1: usize) {
    unsafe {
        asm!(
        "svc 0",
        in("r0") id,
        in("r1") a0,
        in("r2") a1,
        options(nostack)
        );
    }
}
