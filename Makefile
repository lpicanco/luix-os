override IMAGE_NAME := target/luix-os
override LIMINE_DIR := target/limine
override LIMINE_TARGET := $(LIMINE_DIR)/limine
override OMVF_DIR := target/ovmf
override TARGET := x86_64-unknown-none
override PROFILE := dev

# Convenience macro to reliably declare user overridable variables.
define DEFAULT_VAR =
    ifeq ($(origin $1),default)
        override $(1) := $(2)
    endif
    ifeq ($(origin $1),undefined)
        override $(1) := $(2)
    endif
endef

.PHONY: run
run: luix-os
	qemu-system-x86_64 -M q35 -m 2G -hda $(IMAGE_NAME) -serial stdio

.PHONY: run-uefi
run-uefi: $(OMVF_DIR) luix-os
	qemu-system-x86_64 -M q35 -m 2G -bios $(OMVF_DIR)/OVMF.fd -hda $(IMAGE_NAME) -serial stdio

$(OMVF_DIR):
	mkdir -p $(OMVF_DIR)
	cd $(OMVF_DIR) && curl -Lo OVMF.fd https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF.fd

$(LIMINE_TARGET):
	mkdir -p target
	git clone https://github.com/limine-bootloader/limine.git --branch=v7.x-binary --depth=1 $(LIMINE_DIR)
	cd $(LIMINE_DIR) && $(MAKE)

.PHONY: kernel
kernel:
	cargo build --target $(TARGET) --profile $(PROFILE) --package kernel

luix-os: $(LIMINE_TARGET) kernel
	rm -f $(IMAGE_NAME)
	dd if=/dev/zero bs=1M count=0 seek=64 of=$(IMAGE_NAME)
	sgdisk $(IMAGE_NAME) -n 1:2048 -t 1:ef00
	./target/limine/limine bios-install $(IMAGE_NAME)
	mformat -i $(IMAGE_NAME)@@1M
	mmd -i $(IMAGE_NAME)@@1M ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine
	mcopy -i $(IMAGE_NAME)@@1M target/x86_64-unknown-none/debug/kernel ::/boot/kernel
	mcopy -i $(IMAGE_NAME)@@1M kernel/limine.cfg $(LIMINE_DIR)/limine-bios.sys ::/boot/limine
	mcopy -i $(IMAGE_NAME)@@1M $(LIMINE_DIR)/BOOTX64.EFI ::/EFI/BOOT
	mcopy -i $(IMAGE_NAME)@@1M $(LIMINE_DIR)/BOOTIA32.EFI ::/EFI/BOOT

.PHONY: clean
clean:
	rm -rf $(IMAGE_NAME)
	cargo clean
