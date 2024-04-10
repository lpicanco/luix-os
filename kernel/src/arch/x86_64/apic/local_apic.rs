use spin::Once;

use crate::acpi::ACPI;
use crate::trace;

pub static LOCAL_APIC: Once<LocalApic> = Once::new();
pub fn init() {
    let madt = ACPI.get().expect("ACPI not initialized").madt;
    let local_apic = madt
        .iter_entries()
        .find_map(|entry| match entry {
            crate::acpi::madt::MadtEntry::LocalApic(local) => Some(LocalApic {
                local_apic_address: madt.local_apic_address,
                acpi_processor_id: local.acpi_processor_id,
                apic_id: local.apic_id,
            }),
            _ => None,
        })
        .expect("LocalApic not found");

    trace!(
        "lapic_address: {:#X}, lapic_processor_id: {:#X}, lapic_id: {}",
        local_apic.local_apic_address,
        local_apic.acpi_processor_id,
        local_apic.apic_id
    );

    local_apic.init();
    LOCAL_APIC.call_once(|| local_apic);
}

pub fn end_of_interrupt() {
    LOCAL_APIC.get().expect("Local APIC not initialized.").end_of_interrupt();
}

#[repr(u32)]
enum Registers {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    EndOfInterrupt = 0xB0,
    DestinationFormat = 0xD0,
    SpuriousInterrupt = 0x0F0,
    LvtTimer = 0x320,
    // Timer initial count
    InitialCount = 0x380,
    // Timer divide configuration
    DivisorConfig = 0x3E0,
}
pub struct LocalApic {
    local_apic_address: u32,
    acpi_processor_id: u8,
    apic_id: u8,
}

impl LocalApic {
    fn init(&self) {
        // set task priority to 0 to allow all
        self.write(Registers::TaskPriority, 0);

        // Set the spurious interrupt vector register to 0x1FF
        let value = 0x100 | 0xFF; // 0x1FFu32;
        self.write(Registers::SpuriousInterrupt, value);

        // configure timer divide configuration
        let div = 0x03u32; // Divide by 16
        self.write(Registers::DivisorConfig, div);

        // configure timer initial count
        let timer_initial_count = 500_000_00;
        self.write(Registers::InitialCount, timer_initial_count);
    }

    pub fn enable_interrupt(&self, vector: u8) {
        let timer_mode = 1u32; // Periodic mode
        // Periodic mode | Timer vector
        let data = (timer_mode << 17) | vector as u32;
        self.write(Registers::LvtTimer, data);

    }

    pub fn end_of_interrupt(&self) {
        self.write(Registers::EndOfInterrupt, 0);
    }

    fn write(&self, register: Registers, value: u32) {
        unsafe {
            let offset = (self.local_apic_address as u64 + register as u64) as *mut u32;
            core::ptr::write_volatile(offset, value)
        }
    }
}
