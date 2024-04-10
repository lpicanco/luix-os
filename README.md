# Luix Operating System

Luix is a simple operating system that I am developing in Rust for learning purposes.
It is heavily inspired on Unix-like operating systems.

## Features

- 64 Bits
- amd64 and aarch64 support
- Multicore
- ACPI

## Development

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
- [ ] Global Descriptor Table
- [ ] System Calls
- [ ] Task Scheduler
- [ ] File System
- [ ] User Mode
- [ ] Multicore
    - [ ] Booting on multiple cores
    - [ ] Inter-Processor Interrupts(IPI)

## Screenshots

<img src="docs/screenshot_01.png" alt="Luix screenshot"/>
