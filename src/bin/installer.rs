use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect, Select, Sort};
use funcs::{archiso_check, create_sub_volumes, fetch_disk, run_command, run_shell_command, user_selection_write};
use std::io::Write;
use std::fs::File;
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

fn create_and_mount_filesystems(disk: &str) -> std::io::Result<()> {
    let location = "/dev/mapper/arch";
    let boot_part = format!("{}1", disk);

    let subvol_list: Vec<String> = "root btrfs srv snapshots pkg log home"
        .split(' ')
        .map(String::from)
        .collect();
    let subvol_mount_list: Vec<String> = "root btrfs srv pkg log home".split(' ').map(String::from).collect();

    let btrfs_options = "noatime,compress=zstd:1";

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
    // Has to be this specific order, otherwise it will fail in the for(subvol, dir) loop
    let directories = [
        "root",
        "btrfs",
        "srv",
        "var/cache/pacman/pkg",
        "var/log",
        "home",
        "tmp",
        // The following below might not be required after running pacstrap
        "boot",
        "proc",
        "sys",
        "dev",
        "run",
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

    run_command(
        "mount",
        &[
            "-t",
            "btrfs",
            "-o",
            &format!("{}{}", btrfs_options, ",subvolid=5"),
            &location,
            "/mnt/btrfs",
        ],
    )?;

    for (subvol, dir) in subvol_mount_list.iter().zip(directories.iter()) {
        let full_path = format!("{}/{}", base_path, dir);
        match run_command(
            "mount",
            &[
                "-t",
                "btrfs",
                "-o",
                &format!("{}{}", btrfs_options, &format!(",subvol=@{}", subvol)),
                &location,
                &full_path,
            ],
        ) {
            Ok(_) => println!("Mounted subvolume '{}' at '{}'", subvol, full_path),
            Err(e) => println!("Failed to mount subvolume '{}' at '{}': {}", subvol, full_path, e),
        }
    }

    Ok(())
}

fn user_configuration() -> std::io::Result<()> {    
    File::create("/root/user_selections.cfg")?;

    loop {
        let items = vec![
            "Keyboard Layout",
            "Username",
            "Hostname",
            "Select GPU type to install drivers for",
            "nvidia_stream_memory_operations",
            "Configure Intel GPU video acceleration",
            "Disable all CPU mitigations",
            "Install Printer and Scanner support",
            "Install Wi-Fi and Bluetooth support",
            "Continue / Exit",
        ];

        let theme = ColorfulTheme::default();

        let selection = Select::with_theme(&theme)
            .with_prompt("Select the items you want to configure")
            .items(&items)
            .interact()
            .unwrap();

        match items[selection] {
            "Keyboard Layout" => {
                let items = vec![
                    "by", "ca", "cf", "cz", "de", "dk", "es", "et", "fa", "fi", "fr", "gr", "hu", "il", "it", "lt",
                    "lv", "mk", "nl", "no", "pl", "ro", "ru", "sg", "ua", "uk", "us",
                ];
                let keyboard_layout = FuzzySelect::with_theme(&theme)
                    .with_prompt("Select your keyboard layout: ")
                    .items(&items)
                    .interact()
                    .unwrap();

                let line = format!("keyboard_layout=");
                user_selection_write(&keyboard_layout.to_string(), &line)?;
            }
            "Username" => {
                let username = Input::<String>::with_theme(&theme)
                    .with_prompt("\nEnter your username")
                    .interact()
                    .unwrap();

                let line = format!("username=");
                user_selection_write(&username.to_string(), &line)?;
            }
            "Hostname" => {
                let hostname = Input::<String>::with_theme(&theme)
                    .with_prompt("\nEnter your hostname")
                    .interact()
                    .unwrap();

                let line = format!("hostname=");
                user_selection_write(&hostname.to_string(), &line)?;
            }
            "Select GPU type to install drivers for" => {
                let gpu_selected = Select::with_theme(&theme)
                    .with_prompt("\nSelect your GPU")
                    .default(0)
                    .items(&["NVIDIA", "Intel", "AMD"])
                    .interact()
                    .unwrap();

                let line = format!("gpu_selected=");
                user_selection_write(&gpu_selected.to_string(), &line)?;
            }
            "nvidia_stream_memory_operations" => {
                let nvidia_stream_memory_operations = Confirm::with_theme(&theme)
                    .with_prompt("\nEnable Nvidia Stream Memory Operations?")
                    .interact()
                    .unwrap();

                let line = format!("nvidia_stream_memory_operations=");
                user_selection_write(&nvidia_stream_memory_operations.to_string(), &line)?;
            }
            "Configure Intel GPU video acceleration" => {
                let items = vec![
                    "Intel GMA 4500 (2008) up to Coffee Lake's (2017) HD Graphics",
                    "Intel HD Graphics series starting from Broadwell (2014) and newer",
                ];
                let intel_video_accel = Select::with_theme(&theme)
                    .with_prompt("Select your Intel GPU generation")
                    .default(0)
                    .items(&items)
                    .interact()
                    .unwrap();

                let line = format!("intel_video_accel=");
                user_selection_write(&intel_video_accel.to_string(), &line)?;
            }
            "Disable all CPU mitigations" => {
                let no_mitigations = Confirm::with_theme(&theme)
                    .with_prompt("Disable all CPU mitigations?")
                    .interact()
                    .unwrap();

                let line = format!("no_mitigations=");
                user_selection_write(&no_mitigations.to_string(), &line)?;
            }
            "Install Printer and Scanner support" => {
                let printers_and_scanners = Confirm::with_theme(&theme)
                    .with_prompt("Install printer and scanner drivers?")
                    .interact()
                    .unwrap();

                let line = format!("printers_and_scanners=");
                user_selection_write(&printers_and_scanners.to_string(), &line)?;
            }
            "Install Wi-Fi and Bluetooth support" => {
                let wifi_and_bluetooth = Confirm::with_theme(&theme)
                    .with_prompt("Install Wi-Fi and Bluetooth drivers?")
                    .interact()
                    .unwrap();

                let line = format!("hardware_wifi_and_bluetooth=");
                user_selection_write(&wifi_and_bluetooth.to_string(), &line)?;
            }
            "Continue / Exit" => {
                break Ok(());
            }
            _ => {
                eprintln!("Invalid selection: {}", items[selection]);
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    let disk = checks();
    let disk_str: &str = match disk {
        Ok(ref s) => s,
        Err(e) => {
            eprintln!("Failed to get 'disk' string: {}", e);
            return Err(e);
        }
    };

    user_configuration()?;

    let set_ntp = run_shell_command("timedatectl set-ntp true");
    match set_ntp {
        Ok(_) => println!("NTP enabled successfully"),
        Err(e) => {
            eprintln!("Failed to enable NTP: {}", e);
            return Err(e);
        }
    }

    let restart_ntp = run_shell_command("systemctl restart systemd-timesyncd.service");
    match restart_ntp {
        Ok(_) => println!("NTP service restarted"),
        Err(e) => {
            eprintln!("Failed to restart NTP service: {}", e);
            return Err(e);
        }
    }
    if let Err(e) = create_and_mount_filesystems(disk_str) {
        eprintln!("create_and_mount_filesystems failed: {}", e);
        return Err(e);
    }

    let _ = fs::remove_file("/mnt/var/lib/pacman/db.lck");
    run_shell_command("pacstrap -K /mnt cryptsetup dosfstools btrfs-progs base base-devel git zsh grml-zsh-config --noconfirm --ask=4 --needed")?;

    if cfg!(debug_assertions) {
        fs::copy(
            "/media/sf_arch-flux-c/target/debug/post_chroot",
            "/mnt/root/post_chroot",
        )?;
    } else {
        fs::copy("/root/post_chroot", "/mnt/root/post_chroot")?;
    }

    fs::copy("/root/selected_disk.cfg", "/mnt/root/selected_disk.cfg")?;

    run_shell_command("chmod +x /mnt/root/post_chroot")?;
    run_shell_command("arch-chroot /mnt /bin/bash -c '/root/post_chroot'")?;

    Ok(())
}
