use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::RwLock;

use crate::drivers::nvme::controller::NvmeController;
use crate::drivers::pci::PCI_DRIVER;
use crate::{pci_device, println};

mod command;
mod controller;
mod queue;

pub(crate) static NVME_CONTROLLERS: RwLock<Vec<Arc<NvmeController>>> = RwLock::new(Vec::new());
pub(crate) fn init() {
    const SUBCLASS_NVME: u8 = 0x08;

    pci_device!()
        .storage_devices()
        .filter(|device| device.subclass == SUBCLASS_NVME)
        .for_each(|device| {
            device.enable_bus_mastering();
            device.enable_mmio();

            let mut controller = NvmeController::new(device.read_bar0_memory_address());
            controller.init();
            println!(
                "NVMe Controller initialized. Total capacity: {}MB. Serial number: {}",
                controller.namespaces[0].size / 1024 / 1024,
                controller
                    .identify_controller
                    .as_ref()
                    .unwrap()
                    .serial_number
            );

            NVME_CONTROLLERS.write().push(Arc::new(controller));
        });
}
