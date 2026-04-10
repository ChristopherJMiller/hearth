#!/usr/bin/env bash
# dev/run-vm.sh — Boot a previously-enrolled VM from its installed disk.
#
# Usage:
#   bash dev/run-vm.sh <vm-name>
#   # or: just start-vm <vm-name>
#
# This boots the existing dev/vms/<name>.qcow2 WITHOUT the enrollment ISO, so
# you go straight into the installed NixOS system. If the disk doesn't exist,
# run `just enroll <name>` first.

set -euo pipefail

NAME="${1:?usage: run-vm.sh <vm-name>  (e.g. just start-vm demo)}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DISK_PATH="$SCRIPT_DIR/vms/${NAME}.qcow2"

if [ ! -f "$DISK_PATH" ]; then
    echo "ERROR: No disk at $DISK_PATH" >&2
    echo "       Run 'just enroll $NAME' first to create and install it." >&2
    exit 1
fi

echo "==> Booting VM '$NAME'"
echo "    Disk: $DISK_PATH"

# --- Locate OVMF firmware for EFI boot ---
OVMF_CODE=""
for candidate in \
    /usr/share/OVMF/OVMF_CODE.fd \
    /usr/share/edk2/ovmf/OVMF_CODE.fd \
    /usr/share/edk2-ovmf/x64/OVMF_CODE.fd \
    /usr/share/qemu/OVMF_CODE.fd \
    "$(nix eval --raw nixpkgs#OVMF.fd 2>/dev/null)/FV/OVMF_CODE.fd" \
    ; do
    if [ -f "$candidate" ]; then
        OVMF_CODE="$candidate"
        break
    fi
done

if [ -z "$OVMF_CODE" ]; then
    echo "WARNING: OVMF firmware not found, falling back to BIOS boot"
    EFI_ARGS=()
else
    echo "    OVMF: $OVMF_CODE"
    EFI_ARGS=(
        -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE"
    )
fi

echo "==> Starting QEMU..."
echo ""

exec qemu-system-x86_64 \
    "${EFI_ARGS[@]}" \
    -m 4G \
    -smp 2 \
    -enable-kvm \
    -drive "file=$DISK_PATH,format=qcow2,if=virtio" \
    -netdev user,id=net0,hostfwd=tcp::2222-:22 \
    -device virtio-net-pci,netdev=net0 \
    -vga virtio \
    -display gtk \
    -name "Hearth VM: $NAME"
