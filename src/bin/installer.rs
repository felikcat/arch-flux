use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, FuzzySelect, Input, Select};
use funcs::{archiso_check, config_write, create_sub_volumes, fetch_disk, run_command, run_shell_command};
use regex::Regex;
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

    // Check if there's already a Btrfs file system
    if !Command::new("lsblk")
        .args(&["-o", "FSTYPE", &location])
        .output()?
        .stdout
        .windows(5)
        .any(|window| window == b"btrfs")
    {
        run_command("mkfs.btrfs", &[&location])?;
        run_command("mkfs.fat", &["-F", "32", &boot_part])?;
    }
    let opts = format!("defaults,noatime,compress=zstd:1");

    let _ = fs::create_dir("/mnt");
    let _ = run_command("umount", &["-flR", "/mnt"]);

    fs::remove_dir_all("/mnt")?;
    fs::create_dir("/mnt")?;

    let root_mount = format!("mount -t btrfs -o {} {} /mnt", &opts, &location);
    if let Err(e) = run_shell_command(&root_mount) {
        eprintln!("Failed to mount root: {}", e);
        process::exit(1);
    }

    // Must be ran after btrfs -> /mnt is mounted
    let base_path = "/mnt";
    // Has to be this specific order, otherwise it will fail in the for(subvol, dir) loop
    let directories = [
        "root",
        "btrfs",
        "srv",
        "var/cache/pacman/pkg",
        "var/log",
        "home",
        // The following below might not be required after running pacstrap
        "tmp",
        "boot",
        "proc",
        "sys",
        "dev",
        "run",
        "etc",
    ];

    for dir in directories.iter() {
        let full_path = format!("{}/{}", base_path, dir);
        match fs::create_dir_all(&full_path) {
            Ok(_) => println!("Created directory: {}", full_path),
            Err(e) => println!("Failed to create directory '{}': {}", full_path, e),
        }
    }

    create_sub_volumes(&subvol_list)?;

    let boot_mount = format!("-t vfat -o nodev,nosuid,noexec {} /mnt/boot", &boot_part);
    run_command("mount", &[&boot_mount])?;
    println!("Mounted boot partition");

    let btrfs_mount = format!("-t btrfs -o {},subvolid=5 {} /mnt/btrfs", &location, &opts);
    run_command("mount", &[&btrfs_mount])?;
    println!("Mounted btrfs subvolume");

    for (subvol, dir) in subvol_mount_list.iter().zip(directories.iter()) {
        let full_path = format!("{}/{}", base_path, dir);
        let subvol_mount = format!("-t btrfs -o {} subvol=@{} {} {}", &opts, &subvol, &location, &full_path);
        run_command("mount", &[&subvol_mount])?;
        println!("Mounted subvolume: {}", subvol);
    }

    Ok(())
}

fn user_configuration() -> std::io::Result<()> {
    if !std::path::Path::new("/root/user_selections.cfg").exists() {
        let contents = "keyboard_layout=us
username=admin
password=CHANGEME
hostname=arch
gpu_selected=1
nvidia_stream_memory_operations=false
intel_video_accel=0
no_mitigations=false
printers_and_scanners=true
hardware_wifi_and_bluetooth=true\n";
        std::fs::write("/root/user_selections.cfg", contents)?;
    }
    let items = vec![
        "Keyboard Layout",
        "Username",
        "Password",
        "Hostname",
        "Select GPU type to install drivers for",
        "nvidia_stream_memory_operations",
        "Configure Intel GPU video acceleration",
        "Disable all CPU mitigations",
        "Printer and Scanner support",
        "Wi-Fi and Bluetooth support",
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
                "by", "ca", "cf", "cz", "de", "dk", "es", "et", "fa", "fi", "fr", "gr", "hu", "il", "it", "lt", "lv",
                "mk", "nl", "no", "pl", "ro", "ru", "sg", "ua", "uk", "us",
            ];
            let keyboard_layout_index = FuzzySelect::with_theme(&theme)
                .with_prompt("Select your keyboard layout: ")
                .items(&items)
                .interact()
                .unwrap();

            let keyboard_layout = &items[keyboard_layout_index];

            let line = format!("keyboard_layout=");
            config_write(&keyboard_layout.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Username" => {
            let username = Input::<String>::with_theme(&theme)
                .with_prompt("\nEnter your username")
                .interact()
                .unwrap();

            let line = format!("username=");
            config_write(&username.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Password" => {
            let password = Input::<String>::with_theme(&theme)
                .with_prompt("\nEnter your username's password")
                .interact()
                .unwrap();

            let line = format!("password=");
            config_write(&password.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Hostname" => {
            let hostname = Input::<String>::with_theme(&theme)
                .with_prompt("\nEnter your hostname")
                .interact()
                .unwrap();

            let line = format!("hostname=");
            config_write(&hostname.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Select GPU type to install drivers for" => {
            let gpu_selected = Select::with_theme(&theme)
                .with_prompt("\nSelect your GPU")
                .default(0)
                .items(&["NVIDIA", "Intel", "AMD"])
                .interact()
                .unwrap();

            let line = format!("gpu_selected=");
            config_write(&gpu_selected.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "nvidia_stream_memory_operations" => {
            let nvidia_stream_memory_operations = Confirm::with_theme(&theme)
                .with_prompt("\nEnable Nvidia Stream Memory Operations?")
                .interact()
                .unwrap();

            let line = format!("nvidia_stream_memory_operations=");
            config_write(
                &nvidia_stream_memory_operations.to_string(),
                &line,
                "/root/user_selections.cfg",
            )?;
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
            config_write(&intel_video_accel.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Disable all CPU mitigations" => {
            let no_mitigations = Confirm::with_theme(&theme)
                .with_prompt("Disable all CPU mitigations?")
                .interact()
                .unwrap();

            let line = format!("no_mitigations=");
            config_write(&no_mitigations.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Printer and Scanner support" => {
            let printers_and_scanners = Confirm::with_theme(&theme)
                .with_prompt("Install printer and scanner drivers?")
                .interact()
                .unwrap();

            let line = format!("printers_and_scanners=");
            config_write(&printers_and_scanners.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Wi-Fi and Bluetooth support" => {
            let wifi_and_bluetooth = Confirm::with_theme(&theme)
                .with_prompt("Install Wi-Fi and Bluetooth drivers?")
                .interact()
                .unwrap();

            let line = format!("hardware_wifi_and_bluetooth=");
            config_write(&wifi_and_bluetooth.to_string(), &line, "/root/user_selections.cfg")?;
        }
        "Continue / Exit" => {
            return Ok(());
        }
        _ => {
            eprintln!("Invalid selection: {}", items[selection]);
        }
    }

    user_configuration()
}

fn pacman_mods() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/mnt/etc/pacman.conf";
    let content = fs::read_to_string(path)?;

    let color_regex = Regex::new(r"^#Color")?;
    let content = color_regex.replace_all(&content, "Color");

    let parallel_downloads_regex = Regex::new(r"^#ParallelDownloads")?;
    let content = parallel_downloads_regex.replace_all(&content, "ParallelDownloads");

    fs::write(path, content.as_ref())?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk = checks();
    let disk_str: &str = match disk {
        Ok(ref s) => s,
        Err(e) => {
            eprintln!("Failed to get 'disk' string: {}", e);
            return Err(Box::new(e));
        }
    };

    user_configuration()?;

    let set_ntp = run_shell_command("timedatectl set-ntp true");
    match set_ntp {
        Ok(_) => println!("NTP enabled successfully"),
        Err(e) => {
            eprintln!("Failed to enable NTP: {}", e);
            return Err(Box::new(e));
        }
    }

    let restart_ntp = run_shell_command("systemctl restart systemd-timesyncd.service");
    match restart_ntp {
        Ok(_) => println!("NTP service restarted"),
        Err(e) => {
            eprintln!("Failed to restart NTP service: {}", e);
            return Err(Box::new(e));
        }
    }
    if let Err(e) = create_and_mount_filesystems(disk_str) {
        eprintln!("create_and_mount_filesystems failed: {}", e);
        return Err(Box::new(e));
    }

    // Account for Pacman suddenly exiting (due to the user sending SIGINT by pressing Ctrl + C).
    let _ = fs::remove_file("/mnt/var/lib/pacman/db.lck");

    run_shell_command("pacstrap -K /mnt cryptsetup dosfstools btrfs-progs base base-devel git zsh grml-zsh-config reflector --noconfirm --ask=4 --needed")?;

    pacman_mods()?;

    let fstab_content = "# Static information about the filesystems.\n\
                        # See fstab(5) for details.\n\
                        \n\
                        # <file system> <dir> <type> <options> <dump> <pass>\n";
    fs::write("/mnt/etc/fstab", fstab_content)?;
    run_shell_command("genfstab -U /mnt >>/mnt/etc/fstab")?;

    fs::copy("/root/user_selections.cfg", "/mnt/root/user_selections.cfg")?;
    fs::copy("/root/selected_disk.cfg", "/mnt/root/selected_disk.cfg")?;

    if cfg!(debug_assertions) {
        let _ = fs_extra::dir::copy("/media/sf_arch-flux", "/mnt/root", &fs_extra::dir::CopyOptions::new());
        run_shell_command("arch-chroot /mnt /bin/bash -c '/root/sf_arch-flux/target/debug/post_chroot'")?;
    } else {
        let _ = fs_extra::dir::copy("/root", "/mnt/root", &fs_extra::dir::CopyOptions::new());
        run_shell_command("arch-chroot /mnt /bin/bash -c '/root/post_chroot'")?;
    }

    Ok(())
}
