#![feature(naked_functions, asm)]
#![no_std]
#![no_main]


pub mod thread;
pub mod scheduler;
pub mod interrupts;
mod asm;
mod stack;
mod memory;

use cortex_m::peripheral::scb::SystemHandler;
use muos_syscall::register;
use muos_syscall::numbers::{SCHEDULER_BOOT, YIELD_NOW, EXIT_THREAD, SLEEP_MS};
use crate::asm::{do_setup};
use crate::memory::{mpu_init_static, mpu_program_thread};

pub(crate) const SYSTICK_FREQ_MS: u32 = 10; // 10 ms ticks

// init the scheduler & install our syscall handlers
pub fn init(clock_freq: u32, core_periph: &mut cortex_m::peripheral::Peripherals) {
    unsafe {
        core_periph.SCB.set_priority(SystemHandler::PendSV, 0xFF);
    }

    init_systick(clock_freq, core_periph);
    scheduler::init_scheduler();
    install_syscalls();
}

pub fn boot() {
    unsafe {
        mpu_init_static();

        cortex_m::interrupt::enable();
        muos_syscall::scheduler_boot();
    }
}

fn install_syscalls() {
    const HANDLERS: &[(usize, unsafe extern "C" fn(usize, usize, usize))] = &[
        (SCHEDULER_BOOT, boot_handler),
        (YIELD_NOW, yield_handler),
        (SLEEP_MS, sleep_ms_handler),
        (EXIT_THREAD, exit_handler),
    ];

    for &(id, handler) in HANDLERS {
        register(id, handler);
    }
}

fn init_systick(clock_freq: u32, core_periph: &mut cortex_m::peripheral::Peripherals) {
    let mut syst = &mut core_periph.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload((clock_freq * SYSTICK_FREQ_MS / 1000) - 1);

    syst.clear_current();
    syst.enable_interrupt();
    syst.enable_counter();
}

unsafe extern "C" fn boot_handler(_: usize, _: usize, _: usize) {
    defmt::trace!("boot handler");
    let (psp, ctrl, eret, stack_base, stack_size) =
        scheduler::with_scheduler(|s| {
            let (psp, ctrl, eret) = s.get_initial_thread_registers();
            let (stack_base, stack_size) = s.get_current_thread_stack();

            (psp, ctrl, eret, stack_base, stack_size)
        });

    mpu_program_thread(stack_base, stack_size);
    do_setup(psp, ctrl, eret)
}

unsafe extern "C" fn yield_handler(_: usize, _: usize, _: usize) {
    defmt::trace!("yield handler");
    cortex_m::peripheral::SCB::set_pendsv();
}

unsafe extern "C" fn sleep_ms_handler(ms: usize, _: usize, _: usize) {
    defmt::trace!("sleep_ms handler: {}", ms);
    scheduler::with_scheduler(|sched| sched.syscall_sleep_ms(ms));
    cortex_m::peripheral::SCB::set_pendsv();
}

unsafe extern "C" fn exit_handler(_: usize, _: usize, _: usize) {
    defmt::trace!("exit handler");
    scheduler::with_scheduler(|sched| sched.syscall_exit_thread());
    cortex_m::peripheral::SCB::set_pendsv();
}
