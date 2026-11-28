# Kernel core extraction (LUIX-46)

`kernel_core` holds Luix's runtime unsafe code. `kernel` depends on it, and the core
has no dependency on kernel policy. The module name remains `memory`. This implements
the extraction milestone in design revision 3. LUIX-47 covers the device framework.

## Module comparison

| Before | After | Reason |
| --- | --- | --- |
| `kernel::arch` | private `kernel_core::arch` | CPU instructions, entry and return state, interrupts and paging |
| `kernel::memory` | private `kernel_core::memory` | Frame and heap ownership, raw mappings |
| `kernel::acpi` | private `kernel_core::acpi` | Retained firmware memory and checked table parsing |
| `kernel::display`, `serial`, `print` | core display and serial modules with safe formatting macros | Framebuffer ownership and port I/O |
| `kernel::drivers::{pci,nvme,virtio_blk}` | private `kernel_core::legacy_drivers` | Trusted hardware and DMA implementations awaiting migration |
| `kernel::drivers::BlockDevice` | `kernel_core::block::BlockDevice`, also exported by kernel | Shared blocking interface over initialized bytes |
| `kernel::drivers::fs` | stays in kernel | GPT, FAT and filesystem policy use safe byte operations |
| `kernel::process` | parsing and loading policy stay in kernel, mechanisms move to `kernel_core::user` | Core owns mappings and return state |
| `kernel::syscall` | dispatch stays in kernel, trap entry and checked copying move to core | Core validates memory access from syscall arguments |
| `kernel::drivers::keyboard`, `utils::ring_buffer` | stay in kernel | Checked enum conversion and `Option<T>` storage |
| kernel entry, panic and test harness | core entry macro, panic handler and shared guest harness | Common startup for production and test images |

Moved modules contain both safe helpers and unsafe operations. Helpers stay with the
resource whose ownership, bounds or lifetime they enforce.

## Public contracts

`#[kernel_core::entry]` exports a safe `fn() -> !` as the boot entry. Its audited
expansion emits the entry symbol and a safe function call. Normal and guest entry
points call `boot::initialize(dispatch)`. Initialization runs once. Repeated calls
preserve the existing allocator, mappings, devices and syscall callback.

`block::devices()` returns shared safe `BlockDevice` implementations. Reads accept
initialized `&mut [u8]`, including partial sectors. Writes require whole sectors.
Invalid ranges and arithmetic overflow return errors. NVMe retains a transfer limit
of two pages.

`ProcessMemory::allocate()` owns ten zeroed pages in one of 64 disjoint slots.
Nine pages hold the image and one holds the stack. `write` checks the supplied offset
and length within the owned region. Drop unmaps pages and invalidates translations
before releasing payload frames. The core retains page table pages for the boot.

The core creates `SyscallContext`. Kernel policy can inspect scalar arguments, copy
up to 4096 bytes from the current owned region, enter an owned image, or exit to a
saved context. Launches from the kernel authorize only their retained path borrow.
Copies return owned bytes after checking overflow and range membership. The core
sets return selectors, flags and stack bounds. Raw frame fields remain private.

`user::launch` retains its path until the synchronous process tree returns. Nested
user spawns use distinct slots and preserve the parent's state. Reentrant kernel
launches fail. The trap clears DF and aligns the stack before entering Rust.

Console formatting, halt, boot partition IDs and guest diagnostics expose specific
operations through safe APIs. Hardware handles remain private. Panic reporting
bypasses console locks.

## Unsafe implementation invariants

The supported environment is x86_64 with one CPU and coherent DMA. The core relies
on correct bootloader mappings, retained immutable firmware tables, and trusted
built in devices and drivers. Raw frame addresses, free operations, device register
offsets, DMA pointers and privileged return state remain private to the core.

ACPI lengths, checksums and MADT entry bounds are checked before parsing. Firmware
slices refer to retained ACPI or bootloader RAM in the direct map. Framebuffer length
and alignment checks bound its unique pixel slice. Heap allocations use checked
arithmetic and honor byte alignment. DMA allocations own contiguous frames aligned
to a page. Zero initialization requires the private unsafe `Zeroable` contract.

NVMe registers use volatile scalar accesses. DMA records written by devices use
`UnsafeCell` with publication and completion fences. Reads copy into caller storage
after completion. Identify buffers reserve the full transfer of 4096 bytes. Queue
mutexes serialize requests. NVMe waits for reset acknowledgement before releasing
queues. Virtio retains its allocation if reset fails.

Each unsafe operation documents its local requirements. Core denies
`unsafe_op_in_unsafe_fn`, undocumented unsafe blocks, missing public docs and multiple
unsafe operations per block. Two annotated virtio protocol functions use one block
for an exchange over the same exclusively held allocation.

## Enforcement and validation

The `kernel` library, binary and tests forbid unsafe code. `kernel_api` enables user
stubs by default for userland. Kernel dependencies disable that feature. CI compiles
the ABI separately with `-Funsafe_code` to enforce the kernel configuration even when
other workspace builds enable user features.

`cargo xtask check-unsafe` parses source syntax, rejects public unsafe core APIs,
checks resolved dependencies for unreviewed proc macros, builds normal and test
consumers, and checks external consumer fixtures. Only the local entry macro and
existing test macro are allowed. The checker supplements rustc's lint. Changes to
core internals still require a safety review. Runtime dependencies, the compiler and
reviewed macro implementations remain trusted.

`mise run test` runs parser, collection, allocator, firmware and xtask tests on the
host. `mise run test:qemu` boots separate core and kernel test images over NVMe and
virtio-blk. Coverage includes short disk reads, write round trips, failed I/O followed
by valid I/O, filesystem reads, malformed ELF and user arguments, nested spawn and
exit, region reuse and payload frame cleanup. `mise run check`, `mise run lint` and
`mise run fmt:check` cover the repository checks.

Validation on 2026-11-28 used `nightly-2026-11-18`. `mise run check` and `mise run ci`
passed. Host tests passed with 11 kernel, 21 core and 3 xtask tests. Each QEMU backend
passed 44 core and 29 kernel tests. All seven external consumer fixtures produced
the expected compile or rejection result. The normal image mounted the boot
filesystem and ran `init` on both backends. Existing Clippy and dead code warnings
remain.

## Remaining work

Processes share one page table. LUIX-20 covers isolation between processes and
separate page tables. User faults currently halt the system. Fault recovery and
page permissions for each process remain unimplemented. Images retain the fixed
allocation limit and synchronous execution model.

The legacy drivers remain trusted for DMA. LUIX-47 covers IOMMU confinement, device
capabilities, asynchronous queues, cancellation, bounded recovery and moving driver
logic into safe kernel code. A silent device can currently leave a polled request
or reset waiting indefinitely. Hardware validation must still establish DMA
isolation and performance.

ACPI discovery supports RSDT. XSDT and BIOS expansion, SMP and other architectures
remain separate work.
