use crate::acpi::ACPI;
use crate::trace;
use spin::Once;

pub static IO_APIC: Once<IoApic> = Once::new();

pub fn init() {
    let io_apic = ACPI
        .get()
        .expect("ACPI not initialized")
        .madt
        .iter_entries()
        .find_map(|entry| match entry {
            crate::acpi::madt::MadtEntry::IoApic(io_apic) => Some(io_apic),
            _ => None,
        })
        .expect("IOApic not found");
    trace!("IO APIC address: {:#X}, ID: {}", io_apic.io_apic_address as usize, io_apic.io_apic_id);

    let io_apic = IoApic { io_apic_address: io_apic.io_apic_address, io_apic_id: io_apic.io_apic_id };
    IO_APIC.call_once(|| io_apic);
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Irq {
    Timer = 0,
    Keyboard = 1,
    Mouse = 12,
}

#[repr(isize)]
enum Registers {
    IoApicIoRegSel = 0x00,
    IoApicIoWin = 0x10,
}

pub struct IoApic {
    io_apic_address: u32,
    io_apic_id: u8,
}

impl IoApic {
    pub fn redirect(&self, irq: Irq, vector: u32) -> &Self {
        let index = 0x10 + irq as u32 * 2;
        let high_index = index + 1;

        let low_part = (vector & 0xFF) | (1 << 15);
        let high_part = (self.io_apic_id as u32) << 24;

        self.write(index, low_part);
        self.write(high_index, high_part);

        // Unmask the interrupt
        self.write(index, low_part & !(1 << 15));
        self
    }

    fn write(&self, reg: u32, value: u32) {
        let io_apic: *mut u32 = self.io_apic_address as *mut u32;
        unsafe {
            let register_ptr = io_apic.offset(Registers::IoApicIoRegSel as isize);
            register_ptr.write_volatile(reg);

            let value_ptr = io_apic.byte_offset(Registers::IoApicIoWin as isize);
            value_ptr.write_volatile(value);
        }
    }
}
