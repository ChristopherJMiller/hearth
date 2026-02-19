#!/usr/bin/env bash
# dev/run-enrollment.sh — Boot the enrollment ISO in a QEMU VM for testing
#
# Usage:
#   bash dev/run-enrollment.sh
#   # or: just enroll
#
# This script:
#   1. Builds the enrollment ISO via `nix build`
#   2. Creates a 20GB qcow2 virtual disk (simulates target hardware)
#   3. Boots QEMU with EFI, 4GB RAM, 2 CPUs, and user-mode networking
#
# The guest can reach the host API server at http://10.0.2.2:3000
# (QEMU user-mode networking maps 10.0.2.2 to the host).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DISK_PATH="$SCRIPT_DIR/enrollment-disk.qcow2"

# --- Build the enrollment ISO ---
echo "==> Building enrollment ISO..."
ISO_PATH=$(nix build "$REPO_ROOT#enrollment-iso" --no-link --print-out-paths)
echo "    ISO: $ISO_PATH"

# Find the actual .iso file inside the Nix output
ISO_FILE=$(find "$ISO_PATH" -name '*.iso' -type f | head -n1)
if [ -z "$ISO_FILE" ]; then
    echo "ERROR: No .iso file found in $ISO_PATH" >&2
    exit 1
fi
echo "    ISO file: $ISO_FILE"

# --- Create virtual disk (if it doesn't exist) ---
if [ ! -f "$DISK_PATH" ]; then
    echo "==> Creating 20GB virtual disk at $DISK_PATH..."
    qemu-img create -f qcow2 "$DISK_PATH" 20G
else
    echo "==> Using existing virtual disk at $DISK_PATH"
fi

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

# --- Boot QEMU ---
echo "==> Starting QEMU..."
echo "    RAM: 4GB, CPUs: 2"
echo "    Host API reachable from guest at http://10.0.2.2:3000"
echo ""

exec qemu-system-x86_64 \
    "${EFI_ARGS[@]}" \
    -m 4G \
    -smp 2 \
    -enable-kvm \
    -cdrom "$ISO_FILE" \
    -drive "file=$DISK_PATH,format=qcow2,if=virtio" \
    -boot d \
    -netdev user,id=net0,hostfwd=tcp::2222-:22 \
    -device virtio-net-pci,netdev=net0 \
    -vga virtio \
    -display gtk \
    -name "Hearth Enrollment VM"
