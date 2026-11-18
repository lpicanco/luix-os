use crate::arch::{InterruptFrame, Registers};

pub(crate) mod elf;
pub(crate) mod loader;

struct Process {
    base_address: u64,
    parent_frame: InterruptFrame,
    parent_registers: Registers,
}
