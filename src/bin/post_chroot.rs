use funcs::run_command;
use std::io;
use std::process::Command;

mod funcs;

fn main() -> std::io::Result<()> {
    // Incase the keyring expired after installer.rs was run
    let output = Command::new("pacman")
        .args(&["-Sy", "--noconfirm", "--ask=4", "archlinux-keyring"])
        .output()?;

    if !output.status.success() {
        eprintln!("Failed to execute pacman command: {:?}", output);
        std::process::exit(1);
    }

    let output = Command::new("pacman")
        .args(&["-Su", "--noconfirm", "--ask=4"])
        .output()?;

    if !output.status.success() {
        eprintln!("Failed to execute pacman command: {:?}", output);
        std::process::exit(1);
    }

    let file_path = "/etc/locale.gen";
    let file_contents = std::fs::read_to_string(file_path)?;
    let modified_file_contents = file_contents.replace("#en_US.UTF-8 UTF-8", "en_US.UTF-8 UTF-8");
    std::fs::write(file_path, modified_file_contents.as_bytes())?;

    run_command("locale-gen", &[""])?;

    let tz_output = Command::new("curl")
        .args(&["-s", "http://ip-api.com/line?fields=timezone"])
        .output()?;
    if !tz_output.status.success() {
        eprintln!("Failed to execute curl command: {:?}", tz_output);
        std::process::exit(1);
    }
    let tz = String::from_utf8_lossy(&tz_output.stdout);

    /*let systemd_firstboot_output = Command::new("systemd-firstboot")
    .args(&[
        "--keymap",
        &keyboard_layout,
        "--timezone",
        &tz,
        "--locale",
        "en_US.UTF-8",
        "--hostname",
        &hostname,
        "--setup-machine-id",
        "--force",
    ])
    .output()
    .unwrap_or_else(|_| {
        eprintln!("Failed to execute systemd-firstboot command");
        std::process::exit(1);
    });
    */

    Ok(())
}
