use core::mem::MaybeUninit;
use crate::scheduler::MAX_THREADS;

pub(crate) const STACK_SIZE: u32 = 1024; // bytes

#[repr(align(8))]
pub(crate) struct ThreadStack {
    pub(crate) stack: [u8; STACK_SIZE as usize],
}

// Statically allocate the task stacks.
#[link_section = ".uninit.stacks"]
pub(crate) static mut THREAD_STACKS: [ThreadStack; MAX_THREADS] = [
    ThreadStack { stack: [0; STACK_SIZE as usize] },
    ThreadStack { stack: [0; STACK_SIZE as usize] },
    ThreadStack { stack: [0; STACK_SIZE as usize] },
    ThreadStack { stack: [0; STACK_SIZE as usize] },
];
