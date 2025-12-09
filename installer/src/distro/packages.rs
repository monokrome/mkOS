//! Generic package names that get mapped to distro-specific names.
//! This lets us write package lists once and have them work across distros.

pub const CORE_PACKAGES: &[&str] = &[
    "base-system",
    "linux-kernel",
    "linux-firmware",
    "intel-ucode",
    "amd-ucode",
];

pub const BOOT_PACKAGES: &[&str] = &["dracut", "efibootmgr", "sbsigntools"];

pub const CRYPT_PACKAGES: &[&str] = &["cryptsetup", "btrfs-progs"];

pub const NETWORK_PACKAGES: &[&str] = &["dhcpcd", "iwd"];

pub const INIT_S6_PACKAGES: &[&str] = &["s6", "s6-rc", "s6-linux-init"];

pub const WAYLAND_PACKAGES: &[&str] = &[
    "wayland",
    "wayland-protocols",
    "wlroots",
    "xwayland",
    "libinput",
    "mesa",
];

pub const DESKTOP_PACKAGES: &[&str] = &[
    "greetd",
    "greetd-tuigreet",
    "kitty",
    "rofi-wayland",
    "pipewire",
    "wireplumber",
];

pub const FONT_PACKAGES: &[&str] = &["font-hack", "font-noto", "font-noto-emoji"];
