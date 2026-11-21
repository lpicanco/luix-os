use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::BlockDevice;

use crate::drivers::nvme::controller::NvmeController;
use crate::drivers::pci::PCI_DRIVER;
use crate::{pci_device, println};

mod command;
mod controller;
mod queue;

pub(super) fn init() -> Vec<Arc<dyn BlockDevice>> {
    let mut devices: Vec<Arc<dyn BlockDevice>> = Vec::new();
    const SUBCLASS_NVME: u8 = 0x08;

    pci_device!()
        .storage_devices()
        .filter(|device| device.subclass == SUBCLASS_NVME)
        .for_each(|device| {
            device.enable_bus_mastering();
            device.enable_mmio();

            let mut controller = NvmeController::new(device.read_bar0_memory_address());
            if let Err(status) = controller.init() {
                println!(
                    "Skipping NVMe controller after admin command failed (status {status:#x})"
                );
                return;
            }
            let Some(namespace) = controller.namespaces.first() else {
                return;
            };
            println!(
                "NVMe Controller initialized. Total capacity: {}MB. Serial number: {}",
                namespace.size / 1024 / 1024,
                controller
                    .identify_controller
                    .as_ref()
                    .unwrap()
                    .serial_number
            );

            devices.push(Arc::new(controller));
        });
    devices
}
