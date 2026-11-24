use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::nvme::controller::NvmeController;
use crate::drivers::pci::PCI_DRIVER;
use crate::drivers::{BlockDevice, SECTOR_SIZE};
use crate::{pci_device, println};

mod command;
mod controller;
mod queue;

#[cfg(test)]
static TEST_CONTROLLERS: spin::Once<Vec<Arc<NvmeController>>> = spin::Once::new();

pub(super) fn init() -> Vec<Arc<dyn BlockDevice>> {
    let mut controllers = Vec::new();
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
            if !controller.has_usable_namespace() {
                println!("Skipping NVMe controller without a {SECTOR_SIZE}-byte block namespace");
                return;
            }
            let namespace = &controller.namespaces[0];
            println!(
                "NVMe Controller initialized. Total capacity: {}MB. Serial number: {}",
                namespace.size / 1024 / 1024,
                controller
                    .identify_controller
                    .as_ref()
                    .unwrap()
                    .serial_number
            );

            controllers.push(Arc::new(controller));
        });
    #[cfg(test)]
    TEST_CONTROLLERS.call_once(|| controllers.clone());
    controllers
        .into_iter()
        .map(|controller| controller as Arc<dyn BlockDevice>)
        .collect()
}
