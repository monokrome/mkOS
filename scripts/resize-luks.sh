#!/bin/bash
set -euo pipefail

# Resize LUKS+btrfs to free up space
# RUN THIS FROM A LIVE USB - NOT YOUR RUNNING SYSTEM
#
# Usage: ./resize-luks.sh /dev/nvme1n1p2 100G
#   - First arg: LUKS partition
#   - Second arg: Amount of space to FREE (e.g., 100G)

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}Error: Must run as root${NC}"
   exit 1
fi

if [[ $# -lt 2 ]]; then
    echo "Usage: $0 <luks-partition> <space-to-free>"
    echo "Example: $0 /dev/nvme1n1p2 100G"
    exit 1
fi

LUKS_PART="$1"
FREE_SPACE="$2"
MAPPER_NAME="resize_target"

# Safety check - make sure partition exists
if [[ ! -b "$LUKS_PART" ]]; then
    echo -e "${RED}Error: $LUKS_PART is not a block device${NC}"
    exit 1
fi

# Check if already mounted
if mount | grep -q "$LUKS_PART"; then
    echo -e "${RED}Error: $LUKS_PART appears to be mounted. Boot from live USB first!${NC}"
    exit 1
fi

echo -e "${YELLOW}=== LUKS+btrfs Resize Script ===${NC}"
echo ""
echo "Partition:     $LUKS_PART"
echo "Space to free: $FREE_SPACE"
echo ""
echo -e "${RED}WARNING: This will resize your encrypted partition.${NC}"
echo -e "${RED}Make sure you have backups!${NC}"
echo ""
read -p "Type 'yes' to continue: " confirm
if [[ "$confirm" != "yes" ]]; then
    echo "Aborted."
    exit 1
fi

echo ""
echo -e "${GREEN}[1/6] Opening LUKS container...${NC}"
cryptsetup open "$LUKS_PART" "$MAPPER_NAME"
MAPPER_PATH="/dev/mapper/$MAPPER_NAME"

# Get current sizes
LUKS_SIZE_BYTES=$(blockdev --getsize64 "$MAPPER_PATH")
LUKS_SIZE_GB=$((LUKS_SIZE_BYTES / 1024 / 1024 / 1024))

# Parse free space (handle G, T suffixes)
FREE_NUM=$(echo "$FREE_SPACE" | sed 's/[^0-9]//g')
FREE_UNIT=$(echo "$FREE_SPACE" | sed 's/[0-9]//g' | tr '[:lower:]' '[:upper:]')

case "$FREE_UNIT" in
    G) FREE_GB=$FREE_NUM ;;
    T) FREE_GB=$((FREE_NUM * 1024)) ;;
    *) echo "Use G or T suffix (e.g., 100G)"; cryptsetup close "$MAPPER_NAME"; exit 1 ;;
esac

NEW_SIZE_GB=$((LUKS_SIZE_GB - FREE_GB))

echo "Current size: ${LUKS_SIZE_GB}G"
echo "New size:     ${NEW_SIZE_GB}G"
echo "Freeing:      ${FREE_GB}G"
echo ""

echo -e "${GREEN}[2/6] Checking btrfs filesystem...${NC}"
btrfs check --readonly "$MAPPER_PATH"

echo ""
echo -e "${GREEN}[3/6] Mounting btrfs temporarily...${NC}"
MOUNT_POINT=$(mktemp -d)
mount "$MAPPER_PATH" "$MOUNT_POINT"

echo ""
echo -e "${GREEN}[4/6] Shrinking btrfs filesystem to ${NEW_SIZE_GB}G...${NC}"
# btrfs resize can work online
btrfs filesystem resize "${NEW_SIZE_GB}G" "$MOUNT_POINT"

echo ""
echo -e "${GREEN}[5/6] Unmounting and shrinking LUKS container...${NC}"
umount "$MOUNT_POINT"
rmdir "$MOUNT_POINT"

# Shrink LUKS (size in 512-byte sectors)
NEW_SIZE_SECTORS=$((NEW_SIZE_GB * 1024 * 1024 * 2))
cryptsetup resize "$MAPPER_NAME" --size "$NEW_SIZE_SECTORS"

echo ""
echo -e "${GREEN}[6/6] Closing LUKS container...${NC}"
cryptsetup close "$MAPPER_NAME"

echo ""
echo -e "${YELLOW}=== Filesystem shrinking complete ===${NC}"
echo ""
echo "Now you need to shrink the partition itself."
echo "The LUKS container is now ${NEW_SIZE_GB}G."
echo ""
echo "Use gdisk or parted to shrink $LUKS_PART:"
echo "  - New partition end should be at current_start + ${NEW_SIZE_GB}G + ~100M (LUKS header)"
echo ""
echo "Or use this approximate sector count:"
PART_SECTORS=$((NEW_SIZE_SECTORS + 32768))  # Add 16MB for LUKS header
echo "  Partition size in sectors: $PART_SECTORS"
echo ""
echo -e "${GREEN}Done! Don't forget to shrink the partition with gdisk/parted.${NC}"
