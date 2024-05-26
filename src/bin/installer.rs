use funcs::{archiso_check, fetch_disk, run_command};

mod funcs;

fn main() {
    let set_ntp = run_command("timedatectl set-ntp true");
    match set_ntp {
        Ok(_) => println!("NTP enabled successfully"),
        Err(e) => {
            eprintln!("Failed to enable NTP: {}", e);
            return;
        }
    }

    let restart_ntp = run_command("systemctl restart systemd-timesyncd.service");
    match restart_ntp {
        Ok(_) => println!("NTP service restarted"),
        Err(e) => {
            eprintln!("Failed to restart NTP service: {}", e);
            return;
        }
    }

    if let Err(e) = archiso_check() {
        eprintln!("Arch Linux ISO check failed: {}", e);
        return;
    }

    let disk = fetch_disk();
    match disk {
        Ok(_) => println!("Fetched disk information successfully: {:?}", disk),
        Err(e) => {
            eprintln!("Failed to fetch disk information: {}", e);
            return;
        }
    }
}
