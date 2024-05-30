use funcs::{find_option, run_command, run_shell_command, touch_file};
use std::{fs::File, io::Write, path::Path};

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

    let keyboard_layout = find_option("keyboard_layout");
    let hostname = find_option("hostname");

    println!("Timezone is {}", tz);
    println!("Setting keyboard layout to {:?}", keyboard_layout);
    println!("Setting hostname to {:?}", hostname);

    run_command(
        "systemd-firstboot",
        &[
            "--keymap",
            &keyboard_layout.unwrap_or("us".to_string()),
            "--timezone",
            &tz,
            "--locale",
            "en_US.UTF-8",
            "--hostname",
            &hostname.as_ref().unwrap_or(&"arch".to_string()),
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
        hostname.unwrap()
    );
    let mut file = File::create("/etc/hosts")?;
    file.write_all(contents.as_bytes())?;

    Ok(())
}
