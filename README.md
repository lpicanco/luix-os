# Luix Operating System

Luix is a hobby operating system being developed in Rust. 

It is a 64-bit operating system that supports both x86-64 and AArch64 architectures, and it is heavily inspired by POSIX systems.


## Features

- 64 Bits
- amd64 and aarch64 support
- Multicore
- ACPI

## Development

Install [mise](https://mise.jdx.dev), then run `mise install`. This installs the pinned
Rust nightly from `rust-toolchain.toml`, the `x86_64-unknown-none` target, rustfmt, Clippy,
and ShellCheck. A host C toolchain provides the linker for host tests and tools.

Install the host toolchain and QEMU to run the guest tests:

- macOS: Xcode Command Line Tools, then `brew install qemu`
- Debian/Ubuntu: `sudo apt install build-essential qemu-system-x86 ca-certificates`

```sh
mise install
mise run doctor    # check native tools
mise run test:all  # host tests, then guest tests in QEMU
```

`cargo xtask` builds the disk images and supervises QEMU for guest tests. It fetches
the prebuilt `BOOTX64.EFI` and `OVMF.fd` from the `luix-os/limine` and
`luix-os/edk2-nightly` forks at the commits pinned in `tools/xtask/src/firmware.rs`.
`mise run setup` fetches both in advance. The binaries and their license notices are
verified using SHA-256 and cached in `.cache/luix/downloads/`. The cache supports
offline reuse and is kept by `mise run clean`.

`mise run run:bios` builds a separate BIOS image and compiles the Limine BIOS installer
for that image.

Run a task with `mise run <task>`. `cargo run` and `cargo test` also work and boot QEMU.
The old Make targets still work.

| Task | Description |
| --- | --- |
| `doctor` | Check native build and test tools |
| `setup` | Download and verify Limine and OVMF |
| `check` | Type-check the kernel, userland, and test macros |
| `build` | Build the kernel and init |
| `image` | Create `target/images/luix-os.img` |
| `image:test` | Create `target/images/luix-os-test.img` |
| `run` | Boot with UEFI |
| `run:bios` | Boot with BIOS |
| `debug` | Boot with UEFI and wait for GDB on `localhost:1234` |
| `test` | Run host tests |
| `test:build` | Build the guest test kernel |
| `test:qemu` | Build the test image and run guest tests in QEMU |
| `test:all` | Run host and guest tests |
| `fmt`, `fmt:check` | Format, or check formatting |
| `lint` | Run Clippy and ShellCheck |
| `ci` | Check formatting, lint, and run all tests |
| `clean` | Remove build output and disk images |

In the kernel, `#[test]` comes from `tools/test-macros`. Tests marked `#[test]` run on the
host and in QEMU. Tests marked `#[test(target = "qemu")]` run only in QEMU.

## Roadmap

### Bootloader
- [x] Bootloader(powered by Limine)

### Kernel
- [x] Graphic text mode
- [x] Interrupt Descriptor Table
- [x] Basic interrupt handling
- [x] Advanced Configuration and Power Interface(ACPI)
    - [x] Root System Description Table(RSDT)
    - [x] System Description Table(SDT)
    - [x] Multiple APIC Description Table(Madt)
- [ ] Advanced Programmable Interrupt Controller(APIC)
    - [x] Local APIC(Single Processor)
    - [ ] Local APIC(Multiple Processors)
    - [x] IO APIC
- [x] Interrupt handling with stack information
- [ ] Memory management
  - [x] Linked List Allocator
  - [ ] Paging
    - [x] Address Translation
    - [x] Page mapping
    - [x] Page unmapping
    - [ ] Address space switching
  - [x] Physical Memory Manager
    - [x] Memory Allocation
    - [x] Memory Deallocation
  - [x] Virtual Memory Manager
- [ ] Drivers
  - [x] Keyboard
  - [x] Timer
  - [x] PCI
  - [x] Storage
    - [x] NVMe 
- [x] Global Descriptor Table
- [ ] System Calls
- [ ] Task Scheduler
- [ ] File System
  - [x] FAT32 Read Support
  - [ ] Ext2
  - [ ] VFS
    - [x] Read-only VFS for the boot filesystem
- [ ] Process Management
  - [x] Process Creation
  - [ ] Process Termination
  - [ ] Process Scheduling
- [ ] User Mode
- [ ] Syscalls
  - [x] Spawn
- [ ] Multicore
    - [ ] Booting on multiple cores
    - [ ] Inter-Processor Interrupts(IPI)

### User land
- [ ] Init process
  - [ ] Mount root filesystem
  - [ ] Start shell
- [ ] Shell

## Screenshots

<img src="docs/screenshot_01.png" alt="Luix screenshot"/>
