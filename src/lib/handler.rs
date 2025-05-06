use anyhow::{Result, anyhow, bail};
use std::process::Command;
use std::str;

use crate::grub::get_boot_counter;

/// reboots the system if boot_counter is greater than 0 or can be forced too
pub fn handle_reboot(force: bool) -> Result<()> {
    if !force {
        let boot_counter = get_boot_counter("/boot/grub2/grubenv")?;
        if boot_counter <= Some(0) {
            bail!("countdown ended, check greenboot-rollback status")
        };
    }
    log::info!("restarting the system");
    Command::new("systemctl").arg("reboot").status()?;
    Ok(())
}

/// rollback to previous deployment if boot counter is less than 0
pub fn handle_rollback() -> Result<()> {
    let boot_counter = get_boot_counter("/boot/grub2/grubenv")?;
    if boot_counter <= Some(0) {
        log::info!("Greenboot will now attempt to rollback to previous deployment");
        Command::new("bootc").arg("rollback").status()?;
        return Ok(());
    }
    bail!("Rollback not initiated");
}

/// writes greenboot status to motd.d/boot-status
pub fn handle_motd(state: &str) -> Result<()> {
    std::fs::write(
        "/etc/motd.d/boot-status",
        format!("Greenboot {state}.").as_bytes(),
    )
    .map_err(|err| anyhow!("Error writing motd: {}", err))
}
