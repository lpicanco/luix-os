# shellcheck shell=bash disable=SC2034
# Shared paths, pinned boot assets, and helpers for the file tasks in this directory.
# Source this file. It is not executable, so mise does not list it as a task.

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tasks=$root/.mise-tasks
target_dir=${CARGO_TARGET_DIR:-$root/target}
bin=$target_dir/x86_64-unknown-none/debug
images=$target_dir/images
cache=$root/.cache/luix/downloads

limine_rev=ca3a5b20d2501e3819983c5cf838c70c30ee532a
limine_sha256=6deee1a3d1098c44fc87373be1c3056e3b5586dc8f6b3fec23e51a49cfe62619
limine=$cache/limine-$limine_rev

ovmf_rev=b7099c7daab0ac05a8fdf1fd0dcb21212d3c9f77
ovmf_sha256=90a1f21fafb839cd73859c999b78091ca9da03996182c184cb17ebec1cdc17b7
ovmf=$cache/OVMF.fd

normal_image=$images/luix-os.img
test_image=$images/luix-os-test.img

qemu=(qemu-system-x86_64 -M q35 -serial stdio -snapshot)
qemu_uefi=("${qemu[@]}" -m 128M -bios "$ovmf" -smp 1 -device "nvme,drive=disk,serial=feedcafe")

# Print the QEMU -drive value that attaches an image to the NVMe device.
nvme_drive() { echo "id=disk,file=$1,format=raw,if=none"; }

checksum() {
    if command -v sha256sum >/dev/null; then
        sha256sum "$1"
    else
        shasum -a 256 "$1"
    fi | cut -d ' ' -f1
}

# Download a file into the cache unless a copy with the expected SHA-256 is already there.
download() {
    local url=$1 file=$2 digest=$3
    if [[ -f "$cache/$file" && $(checksum "$cache/$file") == "$digest" ]]; then
        return
    fi
    mkdir -p "$cache"
    stage=$(mktemp -d "$cache/.stage.XXXXXX")
    trap 'rm -rf "$stage"' EXIT
    curl --fail --location --silent --show-error --retry 2 "$url" -o "$stage/$file"
    if [[ $(checksum "$stage/$file") != "$digest" ]]; then
        echo "Invalid SHA-256 for $file" >&2
        return 1
    fi
    mv "$stage/$file" "$cache/$file"
}

# Build a GPT disk image with one FAT32 partition holding Limine, the kernel, and init.
# Usage: build_image <output> <size-mib> <kernel> [extra sgdisk args...]
# Add more files with fat_copy and mmd "${fat[@]}", then call finish_image.
build_image() {
    local output=$1 size_mib=$2 kernel=$3
    shift 3
    mkdir -p "$images"
    image_tmp=$(mktemp -d "$images/.image.XXXXXX")
    trap 'rm -rf "$image_tmp"' EXIT
    disk=$image_tmp/disk

    dd if=/dev/zero of="$disk" bs=1048576 count=0 seek="$size_mib" 2>/dev/null
    # sgdisk --quiet segfaults on some builds, so discard its output instead.
    sgdisk "$disk" -n 1:2048 -t 1:ef00 "$@" >/dev/null
    "$limine/limine" bios-install "$disk"

    # Size the filesystem to the partition so it stays clear of the backup GPT.
    local last_sector
    last_sector=$(sgdisk -i 1 "$disk" | sed -n 's/^Last sector: \([0-9]*\).*/\1/p')
    fat=(-i "$disk@@1M")
    mformat -F -T $((last_sector - 2048 + 1)) "${fat[@]}"
    mmd "${fat[@]}" ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine

    fat_copy "$kernel" ::/boot/kernel
    fat_copy "$bin/init" ::/boot/init
    fat_copy "$root/kernel/limine.cfg" "$limine/limine-bios.sys" ::/boot/limine
    fat_copy "$limine/BOOTX64.EFI" "$limine/BOOTIA32.EFI" ::/EFI/BOOT
    image_output=$output
}

fat_copy() { mcopy "${fat[@]}" "$@"; }

# Move the image built by build_image into place.
finish_image() {
    mv "$disk" "$image_output"
    echo "$image_output"
}
