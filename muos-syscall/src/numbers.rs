/// Maximum syscall IDs supported.
pub const MAX_SYSCALL_ID: usize = 32;

pub const SCHEDULER_BOOT: usize = 0;
pub const YIELD_NOW: usize = 1;
pub const SLEEP_MS: usize = 2;
pub const EXIT_THREAD: usize = 3;
