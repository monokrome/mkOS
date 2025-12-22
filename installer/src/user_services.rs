//! User-level service infrastructure setup
//!
//! This module sets up the infrastructure for user-level services,
//! delegating to the init system implementation for the actual setup.

use anyhow::Result;
use std::path::Path;

use crate::distro::Distro;

/// Set up user-level service infrastructure
///
/// This creates the necessary directories and scripts in /etc/skel
/// so that new users will have user-level service support.
pub fn setup_user_services(root: &Path, distro: &dyn Distro) -> Result<()> {
    distro.init_system().setup_user_services(root)
}
