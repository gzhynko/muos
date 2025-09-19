#![no_std]

pub type ThreadFn = fn() -> ();

#[derive(Copy, Clone, PartialEq)]
pub enum BlockReason {
    Sleep(usize),
}

#[derive(Copy, Clone, PartialEq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked(BlockReason),
    Exited,
}

#[derive(Copy, Clone)]
pub struct ThreadContext {
    pub stack_addr: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Thread {
    pub context: ThreadContext,
    pub prio: u32,
    pub fn_addr: u32,
    pub privileged: bool,
    pub fp: bool, // whether thread starts with floating point context enabled
    pub state: ThreadState,
}

impl Thread {
    pub fn new(
        stack_addr: u32,
        prio: u32,
        fn_addr: u32,
        privileged: bool,
        fp: bool,
    ) -> Self {
        const CALLEE_REGS_SIZE: u32 = 8 * 4;
        const EXC_FRAME_SIZE: u32 = 8 * 4;
        let stack_top = stack_addr & !0x7;  // enforce 8-byte alignment at top

        // Allocate space for both frames explicitly:
        let frame_start = (stack_top - EXC_FRAME_SIZE) & !0x7;
        let regs_start  = (frame_start - CALLEE_REGS_SIZE) & !0x7;

        assert!(frame_start % 8 == 0 && regs_start % 8 == 0);

        unsafe {
            // clear callee-saved regs
            let mut ptr = regs_start as *mut u32;
            for _ in 0..8 {
                ptr.write(0);
                ptr = ptr.add(1);
            }

            // write initial exception frame
            let frame_ptr = frame_start as *mut u32;
            let frame = [
                fn_addr,            // R0: argument (thread entry fn)
                0,                  // R1
                0,                  // R2
                0,                  // R3
                0,                  // R12
                0xFFFFFFFD,         // LR (return to thread mode using PSP)
                (thread_trampoline as u32) | 1,  // PC (thread entry point)
                0x01000000,         // xPSR (Thumb mode)
            ];

            for (i, &w) in frame.iter().enumerate() {
                frame_ptr.add(i).write(w);
            }
        }

        Thread {
            context: ThreadContext { stack_addr: regs_start },
            prio,
            fn_addr,
            privileged,
            fp,
            state: ThreadState::Ready,
        }
    }

    pub fn from_thread_fn(thread_fn: ThreadFn, stack_addr: u32) -> Self {
        defmt::trace!("thread: from_thread_fn: stack addr: {:#x}", stack_addr);
        Self::new(
            stack_addr,
            0,
            thread_fn as u32,
            false,
            false,
        )
    }

    pub fn get_ctrl(&self) -> u32 {
        if self.privileged {
            0x2
        } else {
            0x3
        }
    }
}

#[no_mangle]
pub extern "C" fn thread_trampoline(f: ThreadFn) {
    //defmt::trace!("thread trampoline start");


    // call the user function
    f();

    // if it ever returns, trap into exit
    muos_syscall::exit_thread();

    defmt::trace!("thread trampoline end");
}
