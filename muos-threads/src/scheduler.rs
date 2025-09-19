#![no_std]

use core::arch::asm;
use core::cell::RefCell;
use cortex_m::interrupt::{self, Mutex};
use crate::{asm, SYSTICK_FREQ_MS};
use crate::stack::{STACK_SIZE, THREAD_STACKS};

use crate::thread::{ThreadState, Thread, ThreadFn, ThreadContext, BlockReason};

pub(crate) const MAX_THREADS: usize = 4;

pub trait Scheduler {
    fn spawn_idle(&mut self, thread_fn: ThreadFn);
    fn spawn(&mut self, thread_fn: ThreadFn);
    fn get_initial_thread_registers(&mut self) -> (u32, u32, u32);
    fn get_current_thread_stack(&self) -> (usize, usize);

    fn schedule(&mut self) -> Option<(*mut ThreadContext, *mut ThreadContext)>;

    fn syscall_sleep_ms(&mut self, ms: usize);
    fn syscall_exit_thread(&mut self);

    fn systick(&mut self);
}

/// The Scheduler holds a fixed array of threads and tracks the current thread.
pub struct RRScheduler {
    pub threads: [Option<Thread>; MAX_THREADS],
    pub current_thread_id: Option<usize>,
    idle_thread_id: Option<usize>,
    tick_count: usize,
}

impl RRScheduler {
    pub fn new() -> Self {
        RRScheduler {
            threads: [None, None, None, None],
            current_thread_id: None,
            idle_thread_id: None,
            tick_count: 0,
        }
    }

    /// Helper: demote curr, promote next, return raw contexts.
    fn do_switch(&mut self, curr: usize, next: usize)
                 -> Option<(*mut ThreadContext, *mut ThreadContext)> {
        // Use split_at_mut to get two distinct &mut slots
        let (prev_slot, next_slot) = if curr < next {
            let (lo, hi) = self.threads.split_at_mut(next);
            (&mut lo[curr], &mut hi[0])
        } else {
            let (lo, hi) = self.threads.split_at_mut(curr);
            (&mut hi[0], &mut lo[next])
        };

        // demote or free prev_slot
        let mut should_free_prev = false;
        match prev_slot.as_mut().unwrap().state {
            ThreadState::Running => prev_slot.as_mut().unwrap().state = ThreadState::Ready,
            ThreadState::Exited  => { should_free_prev = true; },
            _ => {}
        }
        // promote next_slot
        let next_t = next_slot.as_mut().unwrap();
        next_t.state = ThreadState::Running;
        self.current_thread_id = Some(next);

        // pull out contexts
        let prev_ctx = &mut prev_slot.as_mut().unwrap().context as *mut _;
        if should_free_prev {
            *prev_slot = None;
        }

        let next_ctx = &mut next_t.context as *mut _;

        Some((prev_ctx, next_ctx))
    }
}

impl Scheduler for RRScheduler {
    /// Spawn the non-deletable idle thread. Call this before any user threads.
    fn spawn_idle(&mut self, fn_idle: ThreadFn) {
        if self.idle_thread_id.is_some() {
            panic!("Idle thread already spawned");
        }
        let slot = self.threads.iter().position(Option::is_none)
            .expect("No slot for idle thread");
        let stack_addr = unsafe { THREAD_STACKS[slot].stack.as_ptr() as u32 + STACK_SIZE };
        let mut t = Thread::from_thread_fn(fn_idle, stack_addr);
        t.state = ThreadState::Ready;
        self.threads[slot] = Some(t);
        self.idle_thread_id = Some(slot);
    }

    /// Add a new user thread.
    fn spawn(&mut self, thread_fn: ThreadFn) {
        let slot = self.threads.iter().position(Option::is_none)
            .expect("No available thread slot");
        defmt::trace!("spawn: slot: {}", slot);
        let stack_addr = unsafe { THREAD_STACKS[slot].stack.as_ptr() as u32 + STACK_SIZE };
        let t = Thread::from_thread_fn(thread_fn, stack_addr);
        self.threads[slot] = Some(t);
        if self.current_thread_id.is_none() {
            self.current_thread_id = Some(slot);
        }
    }

    /// Round-robin scheduler skipping idle until fallback.
    fn schedule(&mut self) -> Option<(*mut ThreadContext, *mut ThreadContext)> {
        let curr = self.current_thread_id.expect("No current thread");
        let idle = self.idle_thread_id.expect("Idle not spawned");

        // 1) scan other threads first
        for offset in 1..MAX_THREADS {
            let next = (curr + offset) % MAX_THREADS;
            if next == idle { continue; }

            if let Some(th) = &mut self.threads[next] {
                if th.state == ThreadState::Ready {
                    return self.do_switch(curr, next);
                }
            }
        }
        // 2) fallback to idle if ready
        if idle != curr {
            if let Some(idle_t) = &mut self.threads[idle] {
                if idle_t.state == ThreadState::Ready {
                    return self.do_switch(curr, idle);
                }
            }
        }
        None
    }

    fn get_initial_thread_registers(&mut self) -> (u32, u32, u32) {
        let tid = self.current_thread_id.unwrap();
        let thread = self.threads[tid].as_ref().unwrap();

        // stack_addr should already point at the very first word of the 8‑word frame:
        let psp        = thread.context.stack_addr + (8 * 4);
        let control    = thread.get_ctrl();
        let exc_return = 0xFFFFFFFD;

        //defmt::debug!(
        //    "booting: tid={}  psp={:#010x}  ctrl={:#x}  EXC_RETURN={:#x}",
        //    tid, psp, control, exc_return
        //  );

        (psp, control, exc_return)
    }

    fn get_current_thread_stack(&self) -> (usize, usize) { // (stack base, stack size)
        let tid = self.current_thread_id.unwrap();
        let thread = self.threads[tid].as_ref().unwrap();

        defmt::trace!("get stack - tid: {}", tid);
        let stack_base = unsafe {
            THREAD_STACKS[tid].stack.as_ptr() as usize
        };
        (stack_base, STACK_SIZE as usize)
    }

    fn syscall_sleep_ms(&mut self, ms: usize) {
        let tid = self.current_thread_id.unwrap();
        let thread = self.threads[tid].as_mut().unwrap();

        let wakeup_time = self.tick_count + ms;
        thread.state = ThreadState::Blocked(BlockReason::Sleep(wakeup_time));
    }

    fn syscall_exit_thread(&mut self) {
        let curr_id = self.current_thread_id.expect("exit_thread: no current thread");
        self.threads[curr_id].as_mut().unwrap().state = ThreadState::Exited;
    }

    fn systick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(SYSTICK_FREQ_MS as usize);

        // ready all threads that are sleeping but now past their deadline
        for t in self.threads.iter_mut().filter_map(Option::as_mut) {
            if let ThreadState::Blocked(BlockReason::Sleep(deadline)) = t.state {
                if self.tick_count >= deadline {
                    t.state = ThreadState::Ready;
                }
            }
        }
    }
}

// Global scheduler instance, stored in a Mutex/RefCell.
static SCHEDULER: Mutex<RefCell<Option<RRScheduler>>> =
    Mutex::new(RefCell::new(None));

pub fn init_scheduler() {
    interrupt::free(|cs| {
        *SCHEDULER.borrow(cs).borrow_mut() = Some(RRScheduler::new());
    });
    spawn_idle(idle_thread as ThreadFn);
}

/// Helper to access the global scheduler safely.
pub fn with_scheduler<F, R>(f: F) -> R
    where
        F: FnOnce(&mut dyn Scheduler) -> R,
{
    interrupt::free(|cs| {
        let mut sched_ref = SCHEDULER.borrow(cs).borrow_mut();
        let scheduler = sched_ref.as_mut().expect("Scheduler not initialized!");
        f(scheduler)
    })
}

fn spawn_idle(thread_fn: ThreadFn) {
    with_scheduler(|sched| sched.spawn_idle(thread_fn));
}
pub fn spawn_thread(thread_fn: ThreadFn) {
    with_scheduler(|sched| sched.spawn(thread_fn));
}

pub fn yield_now() {
    muos_syscall::yield_now();
}

pub fn sleep_ms(ms: u32) {
    muos_syscall::sleep_ms(ms);
}

pub fn schedule() -> Option<(*mut ThreadContext, *mut ThreadContext)> {
    with_scheduler(|sched| sched.schedule())
}

fn idle_thread() {
    defmt::debug!("ENTERED IDLE THREAD!");
    loop {
        cortex_m::asm::wfi();  // wait for next interrupt
    }
}
