#![no_std]
#![no_main]

use cyw43_min::pio_spi::{PioSpi};
use embedded_hal::digital::PinState;
use {defmt_rtt as _, panic_probe as _};

use rp235x_hal::{self as hal, pio::{PIOExt, PIOBuilder, StateMachine, ShiftDirection}, dma::DMAExt, gpio::{Pin, AnyPin, FunctionSio, PullType, SioOutput}, pac, Clock};

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;

use embedded_hal::spi::SpiBus;
use rp235x_hal::dma::{SingleChannel};
use muos_syscall::sleep_ms;

use muos_threads::scheduler::{spawn_thread, yield_now};
use muos_threads::thread::ThreadFn;

/// Tell the Boot ROM about our application
#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

const XTAL_FREQ_HZ: u32 = 12_000_000u32;

#[hal::entry]
fn main() -> ! {
  // Grab our singleton objects
  let mut pac = hal::pac::Peripherals::take().unwrap();
  let mut core = cortex_m::peripheral::Peripherals::take().unwrap();

  // Set up the watchdog driver - needed by the clock setup code
  let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

  // Configure the clocks
  let clocks = hal::clocks::init_clocks_and_plls(
    XTAL_FREQ_HZ,
    pac.XOSC,
    pac.CLOCKS,
    pac.PLL_SYS,
    pac.PLL_USB,
    &mut pac.RESETS,
    &mut watchdog,
  )
  .unwrap();

  let timer = hal::Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

  let sio = hal::Sio::new(pac.SIO);
  // Set the pins to their default state
  let pins = hal::gpio::Pins::new(
    pac.IO_BANK0,
    pac.PADS_BANK0,
    sio.gpio_bank0,
    &mut pac.RESETS,
  );

  let mut dma = pac.DMA.split(&mut pac.RESETS);
  dma.ch0.enable_irq0();

  let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);
  let clock_pin = pins.gpio29; // WL_CLK
  let data_pin = pins.gpio24; // WL_D
  let cs_pin_func = pins.gpio25.into_push_pull_output_in_state(PinState::High); // WL_CS
  let on_pin_func = pins.gpio23.into_push_pull_output_in_state(PinState::Low); // WL_ON

  let pio_spi = PioSpi::new(&mut pio, sm0, clock_pin, data_pin, dma.ch0, cs_pin_func);
  //let mut cyw43_driver = cyw43_min::new(on_pin_func, pio_spi, timer.clone());

  muos_threads::init(clocks.system_clock.freq().to_Hz(), &mut core);

  //defmt::trace!("before spawn thread 1");
  spawn_thread(thread1 as ThreadFn);
  //defmt::trace!("after spawn thread 1");
  //spawn_thread(thread2 as ThreadFn);
  //defmt::trace!("after spawn thread 2");

  muos_threads::boot();
  defmt::error!("ERROR: still in main after do_setup!");

  loop {
  }
}

fn thread1() {
  defmt::debug!("hello from thread 1");
  sleep_ms(5000);
  defmt::debug!("AFTER from thread 1");
}

fn thread2() {
  defmt::debug!("hello from thread 2");
  //yield_now();
  defmt::debug!("AFTER from thread 2");
  spawn_thread(thread3 as ThreadFn);
}

fn thread3() {
  defmt::debug!("thread 3: yielding");
  //yield_now();
  defmt::debug!("thread 3: inft loop");

  let mut a = 0;
  for _ in 0..1_000 {
    a += 1;
    sleep_ms(1);
  }

  defmt::debug!("thread 3: {}", a);
}

fn thread4() {
  defmt::debug!("THREAD 4");
}

