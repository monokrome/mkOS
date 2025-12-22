//! Legacy UKI module - re-exports from boot module for backwards compatibility
//!
//! New code should use `crate::boot` directly.

mod secureboot;

pub use secureboot::*;

// Re-export types and functions from the new boot module
pub use crate::boot::{
    create_boot_entry, create_startup_script, generate_dracut_config, generate_uki, BootConfig,
};
