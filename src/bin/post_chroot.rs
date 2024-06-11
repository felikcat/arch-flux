use anyhow::Context;
use funcs::{find_option, get_march, replace_text, run_command, run_shell_command, touch_file};
use regex::Regex;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::Path,
    thread,
};

mod funcs;

fn main() -> anyhow::Result<()> {
    // Test the 10 most reliable mirrors, given their last full sync is at max 30 minutes delayed.
    if Path::new("/tmp/skip_reflector").is_file() {
        run_shell_command(
            "reflector --verbose -p https --delay 0.5 --score 10 --fastest 6 --save /etc/pacman.d/mirrorlist",
        )
        .with_context(|| "Failed to update pacman mirrorlist")?;

        touch_file("/tmp/skip_reflector")?;
    }

    // Incase the keyring expired after installer.rs was run
    run_command("pacman", &["-Sy", "--noconfirm", "--ask=4", "archlinux-keyring"])
        .with_context(|| "Failed to update keyring")?;
    run_command("pacman", &["-Su", "--noconfirm", "--ask=4"]).with_context(|| "Failed to update the system")?;

    let file_path = "/etc/locale.gen";
    let file_contents = std::fs::read_to_string(file_path).with_context(|| "Failed to read /etc/locale.gen")?;
    let modified_file_contents = file_contents.replace("#en_US.UTF-8 UTF-8", "en_US.UTF-8 UTF-8");
    std::fs::write(file_path, modified_file_contents.as_bytes())?;

    run_command("locale-gen", &[""])?;

    let tz_output = run_command("curl", &["-s", "http://ip-api.com/line?fields=timezone"])
        .with_context(|| "Failed to retrieve timezone from ip-api.com")?;
    let tz = String::from_utf8_lossy(&tz_output.stdout).trim().to_string();

    let keyboard_layout = find_option("keyboard_layout").unwrap();
    let hostname = find_option("hostname").unwrap();
    let username = find_option("username").unwrap();
    let password = find_option("password").unwrap();
    let printers_and_scanners = find_option("printers_and_scanners").unwrap();
    let wifi_and_bluetooth = find_option("wifi_and_bluetooth").unwrap();

    run_command(
        "systemd-firstboot",
        &[
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
        ],
    )
    .unwrap_or_else(|_| {
        eprintln!("Failed to execute systemd-firstboot command");
        std::process::exit(1);
    });

    run_command("hwclock", &["--systohc"])?;

    let contents = format!(
        "# Static table lookup for hostnames.\n\
        # See hosts(5) for details.\n\n\
        127.0.0.1        localhost\n\
        ::1              ip6-localhost\n\
        127.0.1.1        {}\n",
        &hostname,
    );
    let mut file = File::create("/etc/hosts")?;
    file.write_all(contents.as_bytes())?;

    run_shell_command("groupadd --force -g 385 gamemode")?;

    // Safe to do; if say /home/admin existed, it wouldn't also remove /home/admin.
    _ = run_command("userdel", &[&username]);

    let add_user = format!("useradd -m -G users,wheel,video,gamemode -s /bin/zsh {}", &username);
    run_shell_command(&add_user).with_context(|| format!("Failed to create user: {}", &username))?;

    run_shell_command(&format!("echo {}:{} | chpasswd", &username, &password))?;

    replace_text("/etc/audit/auditd.conf", "log_group = root", "log_group = wheel")
        .with_context(|| "Cannot find /etc/audit/auditd.conf")?;
    replace_text("/etc/sudoers", "# %wheel ALL=(ALL) ALL", "%wheel ALL=(ALL) ALL")
        .with_context(|| "Cannot find /etc/sudoers")?;

    fs::write("/etc/sudoers.d/99-installer", b"%wheel ALL=(ALL) NOPASSWD: ALL\n")?;

    let fontconfig_dir = format!("/home/{}/.config/fontconfig/conf.d", &username);
    let systemd_user_dir = format!("/home/{}/.config/systemd/user", &username);
    let directories = vec![
        "/boot",
        "/etc/conf.d",
        "/etc/fonts",
        "/etc/modprobe.d",
        "/etc/modules-load.d",
        "/etc/NetworkManager/conf.d",
        "/etc/pacman.d/hooks",
        "/etc/snapper/configs",
        "/etc/systemd/coredump.conf.d",
        "/etc/tmpfiles.d",
        "/etc/X11",
        "/usr/share/libalpm/scripts",
        "/usr/lib/modules", // Prevent DKMS module installation failures
        &fontconfig_dir,
        &systemd_user_dir,
    ];

    for dir in directories {
        // create_dir_all is used so all parent directories are created as well
        if let Err(e) = fs::create_dir_all(Path::new(dir)) {
            eprintln!("Failed to create directory '{}': {}", dir, e);
        }
    }

    let num_cpus = thread::available_parallelism().unwrap().get();

    let makepkg_path = "/etc/makepkg.conf";
    let content = fs::read_to_string(makepkg_path)?;

    let march = get_march().unwrap_or("native".to_string());

    println!("Optimizing for CPU: {}", march);

    // march: Optimize for current CPU generation.
    // RUSTFLAGS: Same reason as the above.
    // num_cpus: Ensure multi-threading to drastically lower compilation times for PKGBUILDs.
    // pbzip2, pigz: Multi-threaded replacements for: bzip2, gzip.
    let replacements = [
        (
            r"-march=x86-64 -mtune=generic",
            &format!("-march={} -mtune={}", march, march),
        ),
        (
            r"\.RUSTFLAGS.*",
            &format!(r#"RUSTFLAGS="-C opt-level=2 -C target-cpu=native""#),
        ),
        (
            r"\.MAKEFLAGS.*",
            &format!(r#"MAKEFLAGS="-j{} -l{}""#, num_cpus, num_cpus),
        ),
        (r"xz -c -z -", &format!("xz -c -z -T {}", num_cpus)),
        (r"bzip2 -c -f", &"pbzip2 -c -f".to_string()),
        (r"gzip -c -f -n", &"pigz -c -f -n".to_string()),
        (r"zstd -c -z -q -", &format!("zstd -c -z -q -T{}", num_cpus)),
        (r"lrzip -q", &format!("lrzip -q -p {}", num_cpus)),
    ];
    let mut modified_content = content;

    for (pattern, replacement) in &replacements {
        let re = Regex::new(pattern).unwrap();
        modified_content = re.replace_all(&modified_content, *replacement).to_string();
    }
    fs::write(makepkg_path, modified_content)?;

    // Set the MAKEFLAGS and GNUMAKEFLAGS environment variables to use all available CPU cores
    let systemd_files = vec!["/etc/systemd/system.conf", "/etc/systemd/user.conf"];

    let re = Regex::new(r"(.DefaultEnvironment.*)").unwrap();
    let replacement = format!(
        "DefaultEnvironment=\"GNUMAKEFLAGS=-j{} -l{}\" \"MAKEFLAGS=-j{} -l{}\"",
        num_cpus, num_cpus, num_cpus, num_cpus
    );

    for file_path in systemd_files {
        let contents = fs::read_to_string(file_path)?;
        let new_contents = re.replace(&contents, replacement.as_str());
        let mut file = fs::OpenOptions::new().write(true).truncate(true).open(file_path)?;
        file.write_all(new_contents.as_bytes())?;
    }

    let pacman_path = "/etc/pacman.conf";
    let contents = fs::read_to_string(pacman_path).with_context(|| "Failed to read /etc/pacman.conf")?;
    // Doing a single regex match like this makes it so both [multilib] and Include must be commented out, otherwise this fails.
    let multilib_regex = Regex::new(r"(?s)#(\[multilib\].*?)#(Include.*)").unwrap();
    let modified_contents = multilib_regex.replace(&contents, "$1$2");
    fs::write(pacman_path, modified_contents.as_bytes())?;

    let mut packages = Vec::new();
    let mut services = Vec::new();

    if printers_and_scanners == "true".to_string() {
        let pac_packages = vec![
            "cups",
            "cups-filters",
            "ghostscript",
            "gsfonts",
            "cups-pk-helper",
            "sane",
            "system-config-printer",
            "simple-scan",
        ];
        packages.extend(pac_packages);
    }

    if wifi_and_bluetooth == "true".to_string() {
        let wb_packages = vec!["iwd", "bluez", "bluez-utils"];
        packages.extend(wb_packages);
    }

    let default_packages = vec![
        "efibootmgr",
        "grub",
        "os-prober",
        "plymouth",
        "irqbalance",
        "power-profiles-daemon",
        "thermald",
        "dbus-broker",
        "gamemode",
        "lib32-gamemode",
        "iptables-nft",
        "audit",
        "apparmor",
        "python-notify2",
        "python-psutil",
        "bubblewrap-suid",
        "chrony",
        "dnsmasq",
        "openresolv",
        "libnewt",
        "pigz",
        "pbzip2",
        "strace",
        "usbutils",
        "linux-firmware",
        "gnome-keyring",
        "avahi",
        "nss-mdns",
        "man-db",
        "man-pages",
        "pacman-contrib",
        "mkinitcpio",
        "bat",
        "wget",
        "trash-cli",
        "reflector",
        "rebuild-detector",
        "vim",
        "zsh-completions",
    ];
    let default_services = vec![
        "fstrim.timer",
        "btrfs-scrub@-.timer",
        "irqbalance.service",
        "dbus-broker.service",
        "power-profiles-daemon.service",
        "thermald.service",
        "avahi-daemon.service",
        "chronyd.service",
        "rfkill-unblock@all",
        "apparmor.service",
    ];
    packages.extend(default_packages);
    services.extend(default_services);

    let virt_result = run_command("systemd-detect-virt", &[]).with_context(|| "Failed to detect virtualization")?;
    let virt_output = String::from_utf8_lossy(&virt_result.stdout).trim().to_string();

    match virt_output.as_str() {
        // TODO: Test the code inside "none" on a physical PC
        "none" => {
            let reader = BufReader::new(File::open("/proc/cpuinfo")?);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.starts_with("vendor") {
                        let parts: Vec<&str> = line.split(':').collect();
                        if parts.len() == 1 {
                            let cpu_vendor: &str = parts[1].trim();

                            match cpu_vendor {
                                "AuthenticAMD" => {
                                    println!("AMD CPU detected, adding amd-ucode");
                                    packages.extend(vec!["amd-ucode"])
                                }
                                "GenuineIntel" => {
                                    println!("Intel CPU detected, adding intel-ucode");
                                    packages.extend(vec!["intel-ucode"])
                                }
                                _ => eprintln!("Your CPU vendor is not supported"),
                            };
                            break;
                        }
                    }
                }
            }
        }
        "kvm" => {
            let kvm_packages = vec!["spice-vdagent", "qemu-guest-agent"];
            packages.extend(kvm_packages);
        }
        "vmware" => {
            let vmware_packages = vec!["open-vm-tools", "gtkmm3"];
            let vmware_services = vec!["vmtoolsd.service", "vmware-vmblock-fuse.service"];
            packages.extend(vmware_packages);
            services.extend(vmware_services);
        }
        "oracle" => {
            let oracle_packages = vec!["virtualbox-guest-utils"];
            let oracle_services = vec!["vboxservice.service"];
            packages.extend(oracle_packages);
            services.extend(oracle_services);
        }
        "microsoft" => {
            let microsoft_packages = vec!["hyperv"];
            let microsoft_services = vec![
                "hv_fcopy_daemon.service",
                "hv_kvp_daemon.service",
                "hv_vss_daemon.service",
            ];
            packages.extend(microsoft_packages);
            services.extend(microsoft_services);
        }
        _ => eprintln!("Your virtualization environment is not supported"),
    };

    let package_list = packages.join(" ");
    let pacman_install = format!("pacman -Syuu --quiet --noconfirm --ask=4 --needed {}", &package_list);
    run_shell_command(&pacman_install)?;

    let service_list = services.join(" ");
    let enable_services = format!("systemctl enable {}", &service_list);
    run_shell_command(&enable_services)?;

    fs::copy(
        "/root/arch-flux/files/etc/X11/Xwrapper.config",
        "/etc/X11/XWrapper.config",
    )
    .with_context(|| "Failed to copy Xwrapper.config")?;

    println!("Post-chroot setup complete!");

    Ok(())
}
