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
	qemu-system-x86_64 -M q35 -m 128M -bios $(OMVF_DIR)/OVMF.fd -drive id=disk,file=$(IMAGE_NAME),format=raw,if=none -device nvme,drive=disk,serial=feedcafe -serial stdio -smp 1

.PHONY: run-uefi-test
run-uefi-test: $(OMVF_DIR) luix-os-test
	qemu-system-x86_64 -M q35 -m 128M -bios $(OMVF_DIR)/OVMF.fd -drive id=disk,file=$(IMAGE_NAME),format=raw,if=none -device nvme,drive=disk,serial=feedcafe -serial stdio -smp 1  -device isa-debug-exit,iobase=0xf4,iosize=0x04 -display none || [ $$? -eq 33 ]

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

.PHONY: kernel-test
kernel-test:
	@rm -f target/$(TARGET)/debug/deps/kernel-*
	cargo test --target $(TARGET) --profile $(PROFILE) --package kernel --lib --no-run
	@cp target/$(TARGET)/debug/deps/$$(cd target/x86_64-unknown-none/debug/deps && find kernel-* -maxdepth 1 -perm -111 -type f) target/x86_64-unknown-none/debug/kernel-test

luix-os: $(LIMINE_TARGET) kernel
	rm -f $(IMAGE_NAME)
	dd if=/dev/zero bs=1M count=0 seek=64 of=$(IMAGE_NAME)
	sgdisk $(IMAGE_NAME) -n 1:2048 -t 1:ef00
	./target/limine/limine bios-install $(IMAGE_NAME)
	mformat -F -i $(IMAGE_NAME)@@1M
	mmd -i $(IMAGE_NAME)@@1M ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine
	mcopy -i $(IMAGE_NAME)@@1M target/x86_64-unknown-none/debug/kernel ::/boot/kernel
	mcopy -i $(IMAGE_NAME)@@1M kernel/limine.cfg $(LIMINE_DIR)/limine-bios.sys ::/boot/limine
	mcopy -i $(IMAGE_NAME)@@1M $(LIMINE_DIR)/BOOTX64.EFI ::/EFI/BOOT
	mcopy -i $(IMAGE_NAME)@@1M $(LIMINE_DIR)/BOOTIA32.EFI ::/EFI/BOOT

luix-os-test: $(LIMINE_TARGET) kernel-test
	rm -f $(IMAGE_NAME)
	dd if=/dev/zero bs=1M count=0 seek=42 of=$(IMAGE_NAME)
	sgdisk $(IMAGE_NAME) -n 1:2048 -t 1:ef00 -c 1:luix-os-test
	./target/limine/limine bios-install $(IMAGE_NAME)
	mformat -F -i $(IMAGE_NAME)@@1M
	mmd -i $(IMAGE_NAME)@@1M ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine
	mcopy -i $(IMAGE_NAME)@@1M target/$(TARGET)/debug/kernel-test ::/boot/kernel
	mcopy -i $(IMAGE_NAME)@@1M kernel/limine.cfg $(LIMINE_DIR)/limine-bios.sys ::/boot/limine
	mcopy -i $(IMAGE_NAME)@@1M $(LIMINE_DIR)/BOOTX64.EFI ::/EFI/BOOT
	mcopy -i $(IMAGE_NAME)@@1M $(LIMINE_DIR)/BOOTIA32.EFI ::/EFI/BOOT

	# Files for FAT32 tests
	mcopy -i $(IMAGE_NAME)@@1M README.md ::/boot
	mmd -i $(IMAGE_NAME)@@1M ::/test ::/test/deep ::/test/deep/inside
	mmd -i $(IMAGE_NAME)@@1M ::/long-dir-name
	echo "This is a file inside a long path" > target/deepfile.txt
	mcopy -i $(IMAGE_NAME)@@1M target/deepfile.txt ::/test/deep/inside
	mcopy -i $(IMAGE_NAME)@@1M README.md ::/

.PHONY: clean
clean:
	rm -rf $(IMAGE_NAME)
	cargo clean
