//! Hardware initialization runs once and retains immutable boot metadata.
use core::sync::atomic::{AtomicBool, Ordering};
use limine::request::KernelFileRequest;

static STARTED: AtomicBool = AtomicBool::new(false);
static KERNEL_FILE: KernelFileRequest = KernelFileRequest::new();

/// Initialize privileged mechanisms once, then install the safe syscall dispatcher.
/// Repeated calls do nothing. No raw resource is handed out twice.
pub fn initialize(dispatch: fn(&mut crate::user::SyscallContext<'_>) -> usize) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    // SAFETY: initialization runs on the boot CPU before interrupt sources are enabled.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    crate::display::init();
    crate::println!("Initializing LuixOS kernel core...");
    crate::memory::init();
    crate::acpi::init();
    crate::arch::apic::init();
    crate::arch::gdt::init();
    crate::user::install(dispatch);
    crate::arch::interrupt::init();
    crate::legacy_drivers::init();
    // SAFETY: the IDT and interrupt controllers are now initialized.
    unsafe {
        core::arch::asm!("sti", options(nomem, nostack));
    }
}

/// Boot disk and partition identifiers. No firmware pointer escapes.
pub fn partition_ids() -> Option<(uuid::Uuid, uuid::Uuid)> {
    let file = KERNEL_FILE.get_response()?.file();
    Some((file.gpt_disk_id()?.into(), file.gpt_partition_id()?.into()))
}
