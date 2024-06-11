use libcryptsetup_rs::consts::flags::CryptActivate;
use libcryptsetup_rs::{
    consts::{flags::CryptVolumeKey, vals::EncryptionFormat},
    CryptInit, LibcryptErr,
};
use nix::libc::{self};
use regex::Regex;
use std::fs::{self, remove_file, File};
use std::io::{self, Write};
use std::path::Path;
// Do not use other Result functions!
use core::result::Result;
use std::sync::Mutex;

use crate::funcs::{prompt, run_command, run_shell_command};

mod funcs;

static IS_SSD: Mutex<bool> = Mutex::new(false);
static IS_NVME: Mutex<bool> = Mutex::new(false);
static WRONG_OPTION: Mutex<bool> = Mutex::new(false);
static WRONG_DISK: Mutex<bool> = Mutex::new(false);
static SAID_NO: Mutex<bool> = Mutex::new(false);
static WRONG_PASSWORD: Mutex<bool> = Mutex::new(false);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = fs::create_dir("/root/arch-flux");
    let mut selected_disk = "/dev/null".to_string();

    loop {
        disk_selection(&mut selected_disk);

        if !*WRONG_OPTION.lock().unwrap() && !*WRONG_DISK.lock().unwrap() && !*SAID_NO.lock().unwrap() {
            break;
        }
    }

    disk_editing(&mut selected_disk)?;

    let file_path = "/root/arch-flux/selected_disk.cfg";
    remove_file(file_path).ok();
    
    let mut config = File::create(file_path)?;
    let sd_with_newline = format!("{}\n", selected_disk);
    config.write_all(sd_with_newline.as_bytes())?;

    let disk_type = if *IS_NVME.lock().unwrap() {
        "NVME"
    } else if *IS_SSD.lock().unwrap() {
        "SSD"
    } else {
        "Unknown"
    };
    let result = format!("{}\n", disk_type);
    config.write_all(result.as_bytes())?;

    Ok(())
}

fn disk_selection(selected_disk: &mut String) {
    // Clear terminal
    print!("{esc}c", esc = 27 as char);

    if *WRONG_OPTION.lock().unwrap() {
        println!("NOTICE: Please enter 'y' or 'n'.\n");
    }
    if *WRONG_DISK.lock().unwrap() {
        println!("NOTICE: An invalid disk has been selected, try again.\n")
    }

    *WRONG_OPTION.lock().unwrap() = false;
    *WRONG_DISK.lock().unwrap() = false;

    if let Err(e) = run_shell_command("lsblk -o PATH,MODEL,PARTLABEL,FSTYPE,FSVER,SIZE,FSUSE%,FSAVAIL,MOUNTPOINTS") {
        eprintln!("Failed to list disks, this is important information: {}", e);
        std::process::exit(1)
    }

    let input = prompt("\nExample disks: /dev/sda, /dev/nvme0n1.\nInput your desired disk, then press ENTER: ");

    let ssd = Regex::new(r"/dev/[s,v]d[a-z]").unwrap().find(&input);
    let nvme = Regex::new(r"/dev/(nvme|mmc)([0-9])n1").unwrap().find(&input);

    match (ssd, nvme) {
        (Some(ssd_match), None) => {
            println!("\nSelected SSD disk: {}\n", ssd_match.as_str());
            *selected_disk = ssd_match.as_str().to_string();
            *IS_SSD.lock().unwrap() = true;
        }
        (None, Some(nvme_match)) => {
            println!("\nSelected NVMe disk: {}\n", nvme_match.as_str());
            *selected_disk = nvme_match.as_str().to_string();
            *IS_NVME.lock().unwrap() = true;
        }
        (Some(_), Some(_)) => {
            eprintln!("Both an SSD and NVMe were provided, expected only one.");
            *WRONG_DISK.lock().unwrap() = true;
            return;
        }
        (None, None) => {
            eprintln!("No disk selected, expected either an SSD or NVMe.");
            *WRONG_DISK.lock().unwrap() = true;
            return;
        }
    }

    let input = prompt("Are you sure [y/n]: ");

    match input.to_lowercase().as_ref() {
        "y" if input.len() == 1 => return,
        "n" if input.len() == 1 => {
            *SAID_NO.lock().unwrap() = true;
            return;
        }
        _ => {
            *WRONG_OPTION.lock().unwrap() = true;
            return;
        }
    }
}

fn wipe_disk(device_path: &str) -> io::Result<()> {
    let target = "/mnt";
    match funcs::umount(target, libc::MNT_FORCE | libc::MNT_DETACH) {
        Ok(()) => println!("Unmounted {} successfully", target),
        Err(e) => eprintln!("Error: {}", e),
    }

    let target = "/mnt/archinstall";
    match funcs::umount(target, libc::MNT_FORCE | libc::MNT_DETACH) {
        Ok(()) => println!("Unmounted {} successfully", target),
        Err(e) => eprintln!("Error: {}", e),
    }

    // Close these two LUKS containers if opened prior.
    let _ = run_command("cryptsetup", &["luksClose", "cleanit"]);
    let _ = run_command("cryptsetup", &["luksClose", "arch"]);

    let swap = format!("swapoff {}*", device_path);

    // Ensure swap isn't used, otherwise it cannot be deleted
    let _ = run_shell_command(&swap);
    // Remove disk's partition-table signatures
    let whole_disk: String = format!("wipefs -af {}*", device_path);
    run_shell_command(&whole_disk)?;
    // Remove disk's GPT & MBR data structures
    run_command("sgdisk", &["-Z", &device_path])?;
    Ok(())
}

fn create_partitions(device_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        // These need to be separate variables in order to do the math equation.
        let total_pages: i64 = libc::sysconf(libc::_SC_PHYS_PAGES).try_into().unwrap();
        let page_size: i64 = libc::sysconf(libc::_SC_PAGE_SIZE).try_into().unwrap();
        let total_ram: i64 = (total_pages * page_size) / (1024 * 1024);

        // Create GPT disk 2048 alignment
        run_command("sgdisk", &["-a", "2048", "-o", &device_path])?;

        run_command(
            "sgdisk",
            &[
                "-n 1::+1024M",
                "--typecode=1:ef00",
                "--change-name=1:'BOOTEFI'",
                &device_path,
            ],
        )?;

        let ram = format!("-n 2::+{}", total_ram.to_string());
        run_command("sgdisk", &[&ram, "--typecode=2:8200", &device_path])?;
        run_command(
            "sgdisk",
            &["-n 3::-0", "--typecode=3:8300", "--change-name=3:'ROOT'", &device_path],
        )?;

        // Inform kernel of partition changes
        run_command("partprobe", &[&device_path])?;
    }

    Ok(())
}

fn create_luks2_container(selected_disk: &str) -> Result<(), LibcryptErr> {
    *WRONG_PASSWORD.lock().unwrap() = false;

    let password = funcs::prompt_u8("\nEnter a new password for the LUKS2 container: ");
    let password_check = funcs::prompt_u8("Please repeat your new password: ");

    if password != password_check {
        *WRONG_PASSWORD.lock().unwrap() = true;
        return Err(LibcryptErr::Other("Passwords do not match, try again.".to_string()));
    }

    let mut luks_part = selected_disk.to_string();

    if *IS_NVME.lock().unwrap() == true {
        luks_part.push_str("p3");
    } else if *IS_SSD.lock().unwrap() == true {
        luks_part.push_str("3");
    }

    let luks_part_str: &str = &luks_part;

    let sd = Path::new(luks_part_str);
    let mut device = CryptInit::init(sd)?;

    device.context_handle().format::<()>(
        EncryptionFormat::Luks2,
        ("aes", "xts-plain"),
        None,
        libcryptsetup_rs::Either::Right(512 / 8), // 512bit key
        None,
    )?;

    device
        .keyslot_handle()
        .add_by_key(None, None, &password, CryptVolumeKey::empty())?;

    device.context_handle().load::<()>(None, None)?;
    device.activate_handle().activate_by_passphrase(
        Some("arch"),
        Some(libcryptsetup_rs_sys::CRYPT_ANY_SLOT as u32),
        &password,
        CryptActivate::empty(),
    )?;

    Ok(())
}

fn disk_editing(selected_disk: &str) -> Result<(), Box<dyn std::error::Error>> {
    wipe_disk(selected_disk)?;
    create_partitions(selected_disk)?;

    loop {
        match create_luks2_container(selected_disk) {
            Ok(_) => println!("LUKS2 container successfully created; disk formatting complete!\n"),
            Err(e) => eprintln!("Failed to create LUKS2 container: {:?}", e),
        };

        if !*WRONG_PASSWORD.lock().unwrap() {
            break;
        }
    }
    Ok(())
}
