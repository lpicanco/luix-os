# shellcheck shell=bash disable=SC2034
# Shared paths and QEMU arguments for the file tasks in this directory.
# Source this file. It is not executable, so mise does not list it as a task.

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tasks=$root/.mise-tasks
target_dir=${CARGO_TARGET_DIR:-$root/target}
bin=$target_dir/x86_64-unknown-none/debug
normal_image=$target_dir/images/luix-os.img
bios_image=$target_dir/images/luix-os-bios.img
ovmf=$root/.cache/luix/downloads/OVMF.fd

qemu=(qemu-system-x86_64 -M q35 -serial stdio -snapshot)
qemu_uefi=("${qemu[@]}" -m 128M -bios "$ovmf" -smp 1 -device "nvme,drive=disk,serial=feedcafe")

# Print the QEMU -drive value that attaches an image to the NVMe device.
nvme_drive() { echo "id=disk,file=$1,format=raw,if=none"; }
