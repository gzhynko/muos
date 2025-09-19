use cortex_m::peripheral::{MPU, SCB};

/// From your MEMORY block:
const FLASH_BASE: usize = 0x1000_0000;
const FLASH_SIZE: usize = 2 * 1024 * 1024;  // 2048 KiB

const SRAM_BASE:  usize = 0x2000_0000;
const SRAM_SIZE:  usize = 512 * 1024;       //  512 KiB

/// New: AP & XN are for RBAR, not RLAR
const RBAR_AP_PRIV_RO_USER_RO: u32 = 0b11 << 1;   // Flash
const RBAR_AP_PRIV_RW_USER_NO: u32 = 0b00 << 1;   // SRAM (temp)
const RBAR_AP_PRIV_RW_USER_RW: u32 = 0b01 << 1;   // Stacks
const RBAR_XN:                  u32 = 1 << 0;     // eXecute‑Never

/// AttrIndx0 still lives in RLAR
const RLAR_ATTRIDX0:            u32 = 0 << 1;
const RLAR_ENABLE:              u32 = 1 << 0;

unsafe fn program_region(region: u8, base: usize, size: usize, rbar_ap: u32, xn: bool) {
    let mpu  = &*MPU::PTR;
    let base = base as u32 & !0x1F;               // 32‑byte align
    let limit = (base + size as u32 - 1) & !0x1F; // inclusive

    // --- select region ---
    mpu.rnr.write(region as u32);

    // --- RBAR: base | AP | XN | (shareability=0) ---
    let mut rbar = base | rbar_ap;
    if xn { rbar |= RBAR_XN; }
    mpu.rbar.write(rbar as u32);

    // --- RLAR: limit | AttrIndx | ENABLE ---
    let rlar = limit | RLAR_ATTRIDX0 | RLAR_ENABLE;
    mpu.rlar.write(rlar);
}

/// Static MPU init – no per‐thread slicing yet.
pub unsafe fn mpu_init_static() {
    let mpu = &*MPU::PTR;

    // 1) Turn MPU off while we program…
    mpu.ctrl.write(0);

    // 2) MAIR[0] = normal, WB‐WA…
    mpu.mair[0].write(0xFF);

    mpu.mair[1].write(0x04);

    // 3) Region 0 → FLASH (exec OK, RO for all)
    program_region(0, FLASH_BASE, FLASH_SIZE, RBAR_AP_PRIV_RO_USER_RO, false);

    // 4) Region 1 → SRAM (noexec, PrivRW/UserNA)
    program_region(1, SRAM_BASE, SRAM_SIZE, RBAR_AP_PRIV_RW_USER_NO, true);

    // --- NEW: Region 3 → SIO ------------------------------------------------
    const SIO_BASE:  usize = 0xD000_0000;
    const SIO_SIZE:  usize = 0x4000;        // 16 KiB covers the whole block

    program_region(
        3,
        SIO_BASE,
        SIO_SIZE,
        RBAR_AP_PRIV_RW_USER_RW, // user can read (enough for defmt_rtt)
        true                     // XN – never execute from a peripheral block
    );

    // 6) Enable MemManage faults
    let scb = &*SCB::PTR;
    scb.shcsr.modify(|r| r | (1 << 16)); // MEMFAULTENA

    // 7) Turn MPU back on (PRIVDEFENA=1, ENABLE=1)
    mpu.ctrl.write((1 << 2) | (1 << 0));
}

pub unsafe fn mpu_program_thread(stack_addr: usize, stack_size: usize) {
    // reprogram region 2
    program_region(2, stack_addr, stack_size, RBAR_AP_PRIV_RW_USER_RW, true);
}
