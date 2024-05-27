use funcs::{archiso_check, create_sub_volumes, fetch_disk, run_command, run_shell_command};
use std::{
    fs,
    process::{self, Command, Stdio},
};

mod funcs;

fn checks() -> std::io::Result<String> {
    if let Err(e) = archiso_check() {
        eprintln!("Arch Linux ISO check failed: {}", e);
        process::exit(1);
    }

    let disk = fetch_disk();
    match disk {
        Ok(_) => println!("Fetched disk information successfully: {:?}", disk),
        Err(e) => {
            eprintln!("Failed to fetch disk information: {}", e);
            process::exit(1);
        }
    }

    let cryptsetup_status = Command::new("cryptsetup")
        .args(["status", "arch"])
        .stdout(Stdio::piped()) // To pipe the output to another command
        .spawn()
        .expect("Failed to start cryptsetup command");

    let grep_result = Command::new("grep")
        .args(["-q", "inactive"])
        .stdin(cryptsetup_status.stdout.expect("Failed to get stdout of cryptsetup"))
        .status()
        .expect("Failed to run grep command");

    if grep_result.success() {
        eprintln!("\nERROR: Forgot to mount the LUKS2 partition as the name 'root'?\n");
        process::exit(1);
    }
    disk // Return result directly
}

fn create_filesystems(disk: &str) -> std::io::Result<()> {
    let location = "/dev/mapper/arch";
    let boot_part = format!("{}1", disk);

    let subvol_list: Vec<String> = "root btrfs srv snapshots pkg log home"
        .split(' ')
        .map(String::from)
        .collect();

    let _ = run_command("umount", &["-flRq", "/mnt"]);

    // Check if there's already a Btrfs file system
    if !Command::new("lsblk")
        .args(&["-o", "FSTYPE", &location])
        .output()?
        .stdout
        .windows(5)
        .any(|window| window == b"btrfs")
    {
        run_command("mkfs.btrfs", &[&location])?;
        run_command("mkfs.fat -F 32", &[&boot_part])?;
    }

    run_command("mount", &["-t", "btrfs", &location, "/mnt"])?;

    let base_path = "/mnt";
    let directories = [
        "tmp",
        "boot",
        "btrfs",
        "var/log",
        "var/cache/pacman/pkg",
        "srv",
        "root",
        "home",
    ];

    for dir in directories.iter() {
        let full_path = format!("{}/{}", base_path, dir);
        match fs::create_dir_all(&full_path) {
            Ok(_) => println!("Created directory: {}", full_path),
            Err(e) => println!("Failed to create directory '{}': {}", full_path, e),
        }
    }

    run_command(
        "mount",
        &["-t", "vfat", "-o", "nodev,nosuid,noexec", &boot_part, "/mnt/boot"],
    )?;

    create_sub_volumes(&subvol_list)?;

    Ok(())
}

fn main() {
    let disk = checks();
    let disk_str: &str = match disk {
        Ok(ref s) => s,
        Err(e) => {
            eprintln!("Failed to get string: {}", e);
            return;
        }
    };

    let set_ntp = run_shell_command("timedatectl set-ntp true");
    match set_ntp {
        Ok(_) => println!("NTP enabled successfully"),
        Err(e) => {
            eprintln!("Failed to enable NTP: {}", e);
            return;
        }
    }

    let restart_ntp = run_shell_command("systemctl restart systemd-timesyncd.service");
    match restart_ntp {
        Ok(_) => println!("NTP service restarted"),
        Err(e) => {
            eprintln!("Failed to restart NTP service: {}", e);
            return;
        }
    }
    create_filesystems(disk_str);
}
