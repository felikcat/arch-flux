use funcs::{find_option, get_march, replace_text, run_command, run_shell_command, touch_file};
use regex::Regex;
use std::{
    fs::{self, read_to_string, File},
    io::Write,
    path::Path,
    thread,
};

mod funcs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test the 15 most reliable mirrors, given their last full sync is at max 30 minutes delayed.
    if !Path::new("/root/skip_reflector").exists() {
        run_shell_command(
            "reflector --verbose -p https --delay 0.5 --score 15 --fastest 6 --save /etc/pacman.d/mirrorlist",
        )?;
        touch_file("/root/skip_reflector")?;
    }

    // Incase the keyring expired after installer.rs was run
    run_command("pacman", &["-Sy", "--noconfirm", "--ask=4", "archlinux-keyring"])?;

    run_command("pacman", &["-Su", "--noconfirm", "--ask=4"])?;

    let file_path = "/etc/locale.gen";
    let file_contents = std::fs::read_to_string(file_path)?;
    let modified_file_contents = file_contents.replace("#en_US.UTF-8 UTF-8", "en_US.UTF-8 UTF-8");
    std::fs::write(file_path, modified_file_contents.as_bytes())?;

    run_command("locale-gen", &[""])?;

    let tz_output = run_command("curl", &["-s", "http://ip-api.com/line?fields=timezone"])?;
    let tz = String::from_utf8_lossy(&tz_output.stdout).trim().to_string();

    let keyboard_layout = find_option("keyboard_layout")?;
    let hostname = find_option("hostname")?;
    let username = find_option("username")?;
    let password = find_option("password")?;
    let printers_and_scanners = find_option("printers_and_scanners")?;
    let wifi_and_bluetooth = find_option("wifi_and_bluetooth")?;

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

    format!(
        "useradd -m -G users,wheel,video,gamemode -s /bin/zsh {}",
        &username
    );
    run_shell_command(&format!(
        "echo {}:{} | chpasswd",
        &username,
        &password
    ))?;

    // Remove "password" from the config file
    let file_content = read_to_string("/root/user_selections.cfg")?;
    let filtered_content: Vec<String> = file_content
        .lines()
        .filter(|line| !line.contains("password"))
        .map(|line| line.to_string())
        .collect();
    fs::write("/root/user_selections.cfg", filtered_content.join("\n"))?;

    replace_text("/etc/audit/auditd.conf", "log_group = root", "log_group = wheel")?;
    replace_text("/etc/sudoers", "# %wheel ALL=(ALL) ALL", "%wheel ALL=(ALL) ALL")?;

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

    let path = "/etc/makepkg.conf";
    let content = fs::read_to_string(path)?;

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
    fs::write(path, modified_content)?;

    // Set the MAKEFLAGS and GNUMAKEFLAGS environment variables to use all available CPU cores
    let files = vec!["/etc/systemd/system.conf", "/etc/systemd/user.conf"];

    let re = Regex::new(r"(.DefaultEnvironment.*)").unwrap();
    let replacement = format!(
        "DefaultEnvironment=\"GNUMAKEFLAGS=-j{} -l{}\" \"MAKEFLAGS=-j{} -l{}\"",
        num_cpus, num_cpus, num_cpus, num_cpus
    );

    for file_path in files {
        let contents = fs::read_to_string(file_path)?;
        let new_contents = re.replace(&contents, replacement.as_str());
        let mut file = fs::OpenOptions::new().write(true).truncate(true).open(file_path)?;
        file.write_all(new_contents.as_bytes())?;
    }

    let contents = fs::read_to_string("/etc/pacman.conf")?;
    let multilib_regex = Regex::new(r"(?s)(\[multilib\].*?)#(Include.*)").unwrap();
    let modified_contents = multilib_regex.replace(&contents, "$1$2");
    fs::write(path, modified_contents.as_bytes())?;

    let mut packages = Vec::new();

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

    fs::copy("/root/files/etc/X11/Xwrapper.config", "/etc/X11/XWrapper.config")?;

    for package in packages {
        run_command("pacman", &["-S", "--noconfirm", "--ask=4", package])?;
    }

    println!("Post-chroot setup complete!");

    Ok(())
}
