use cryptsetup_rs::device::*;
use libparted_sys::*;
use nix::libc::{self};
use nix::unistd::close;
use nix::{self, ioctl_read_bad};
use regex::Regex;
use std::ffi::CString;
use std::fmt::{Display, Formatter};
use std::fs::{remove_file, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::ptr::null_mut;
// Do not use other Result functions!
use core::result::Result;
use libcryptsetup_rs::{
    consts::{flags::CryptVolumeKey, vals::EncryptionFormat},
    CryptInit, LibcryptErr,
};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::Mutex;

use crate::funcs::{mib_to_sectors, parted_cleanup, prompt, run_command};

mod funcs;

static SECTOR_SIZE: Mutex<i64> = Mutex::new(0);
static IS_SSD: Mutex<bool> = Mutex::new(false);
static IS_NVME: Mutex<bool> = Mutex::new(false);
static WRONG_OPTION: Mutex<bool> = Mutex::new(false);
static WRONG_DISK: Mutex<bool> = Mutex::new(false);
static SAID_NO: Mutex<bool> = Mutex::new(false);
static WRONG_PASSWORD: Mutex<bool> = Mutex::new(false);

fn main() -> io::Result<()> {
    let mut selected_disk = "/dev/null".to_string();

    loop {
        disk_selection(&mut selected_disk);

        if !*WRONG_OPTION.lock().unwrap() && !*WRONG_DISK.lock().unwrap() && !*SAID_NO.lock().unwrap() {
            break;
        }
    }

    disk_editing(&mut selected_disk);

    let file_path = "/root/selected_disk.cfg";

    if let Err(e) = remove_file(file_path) {
        eprintln!("Failed to delete disk configuration file, this is OK: {}", e);
    }

    let mut config = File::create(file_path)?;
    let sd_with_newline = format!("{}\n", selected_disk);
    config.write_all(sd_with_newline.as_bytes())?;

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

    if let Err(e) = run_command("lsblk -o PATH,MODEL,PARTLABEL,FSTYPE,FSVER,SIZE,FSUSE%,FSAVAIL,MOUNTPOINTS") {
        eprintln!("Failed to list disks, this is important information: {}", e);
        std::process::exit(1)
    }

    let input = prompt("\nExample disks: /dev/sda, /dev/nvme0n1.\nInput your desired disk, then press ENTER: ");

    let ssd = Regex::new(r"/dev/sd[a-z]").unwrap().find(&input);
    let nvme = Regex::new(r"/dev/(nvme|mmc)([0-9])n1").unwrap().find(&input);

    struct RegexMatch<'a>(&'a str); // Store

    impl<'a> Display for RegexMatch<'a> {
        fn fmt(&self, format: &mut Formatter) -> std::fmt::Result {
            write!(format, "{}", self.0)
        }
    }

    match (ssd, nvme) {
        (Some(ssd_str), None) => {
            let regex_match = RegexMatch(ssd_str.as_str());
            println!("\nSelected SSD disk: {}\n", regex_match);
            *selected_disk = regex_match.to_string();
            *IS_SSD.lock().unwrap() = true;
        }
        (None, Some(nvme_str)) => {
            let regex_match = RegexMatch(nvme_str.as_str());
            println!("\nSelected NVMe disk: {}\n", regex_match);
            *selected_disk = regex_match.to_string();
            *IS_NVME.lock().unwrap() = true;
        }
        (Some(_), Some(_)) => {
            eprintln!("Both an SSD and NVMe was provided, expected only one.");
            *WRONG_DISK.lock().unwrap() = true;
            return;
        }
        (None, None) => {
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
    let mut device = OpenOptions::new().read(true).write(true).open(device_path)?;

    // https://github.com/strace/strace/blob/master/src/linux/64/ioctls_inc.h
    ioctl_read_bad!(blksszget, 0x1268, libc::c_int);

    let mut size: i32 = 0;

    let fd: RawFd = device.as_raw_fd();

    unsafe {
        // Get sector size
        blksszget(fd, &mut size).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    };

    let size_i64 = size as i64;

    let mut size_lock = SECTOR_SIZE.lock().unwrap();
    *size_lock = size_i64;

    let buffer_size =
        usize::try_from(size_i64).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid sector size"))?;
    let buffer = vec![0u8; buffer_size];

    device.write_all(&buffer)?; // Write zeros to disk.
    device.sync_all()?; // Ensure writes are flushed to disk.

    close(fd).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to close disk device"))?;

    println!("Wiped disk '{}' successfully", device_path);

    Ok(())
}

// https://www.gnu.org/software/parted/api/modules.html
fn create_partitions(device_path: &str, sector_size: i64) {
    let disk_path = CString::new(device_path).expect("CString::new failed");
    unsafe {
        ped_device_probe_all();

        let device = ped_device_get(disk_path.as_ptr());

        if device.is_null() {
            eprintln!("Failed to open {:?}", device);
            ped_device_destroy(device);
            return;
        }

        let dt = CString::new("gpt").unwrap();
        let disk_type = ped_disk_type_get(dt.as_ptr());
        let disk = ped_disk_new_fresh(device, disk_type);
        if disk.is_null() {
            eprintln!("Failed to create a new disk label");
            ped_device_destroy(device);
            return;
        }

        let device_fields = *device;

        let start = 2048; // Start sector / alignment
        let end = device_fields.length - 1;

        // These need to be separate variables in order to do the math equation.
        let total_pages: i64 = sysconf(libc::_SC_PHYS_PAGES).try_into().unwrap();
        let page_size: i64 = sysconf(libc::_SC_PAGE_SIZE).try_into().unwrap();
        let total_ram: i64 = (total_pages * page_size) / (1024 * 1024);

        println!(
            "sector_size: {} sectors, end: {} sectors, total_ram: {} MiB",
            sector_size, end, total_ram
        );

        let boot = ped_partition_new(
            disk,
            PedPartitionType::PED_PARTITION_NORMAL,
            null_mut(),
            start,
            mib_to_sectors(1024, sector_size),
        ); // 1024MiB
        if boot.is_null() {
            eprintln!("Failed to add the boot partition to disk");
            parted_cleanup(disk, device);
        }
        if ped_disk_add_partition(disk, boot, ped_constraint_any(device)) == 0 {
            eprintln!("Failed to write the boot partition to disk");
            parted_cleanup(disk, device);
        };

        let swap = ped_partition_new(
            disk,
            PedPartitionType::PED_PARTITION_NORMAL,
            null_mut(),
            mib_to_sectors(1024 + 1, sector_size),
            mib_to_sectors(total_ram, sector_size) + mib_to_sectors(1024 + 1, sector_size),
        );
        if swap.is_null() {
            eprintln!("Failed to add the swap partition to disk");
            parted_cleanup(disk, device);
        }
        if ped_disk_add_partition(disk, swap, ped_constraint_any(device)) == 0 {
            eprintln!("Failed to write the swap partition to disk");
            parted_cleanup(disk, device);
        };

        let root = ped_partition_new(
            disk,
            PedPartitionType::PED_PARTITION_NORMAL,
            null_mut(),
            mib_to_sectors(total_ram + 1024 + 2, sector_size).into(),
            end,
        );
        if root.is_null() {
            eprintln!("Failed to add the root partition to disk");
            parted_cleanup(disk, device);
        }
        if ped_disk_add_partition(disk, root, ped_constraint_any(device)) == 0 {
            eprintln!("Failed to write the root partition to disk");
            parted_cleanup(disk, device);
        };

        if ped_disk_commit_to_dev(disk) == 0 {
            eprintln!("Failed to write changes to disk");
            parted_cleanup(disk, device);
        };

        if ped_disk_commit_to_os(disk) == 0 {
            eprintln!("Failed to commit write changes to disk");
            parted_cleanup(disk, device);
        };
    }
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
    device.context_handle().set_label(Some("root"), Some("btrfs"))?;

    device
        .keyslot_handle()
        .add_by_key(None, None, &password, CryptVolumeKey::SET)?;

    Ok(())
}

fn disk_editing(selected_disk: &str) {
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
    File::create("/dev/mapper/cleanit").unwrap();
    let path = init("/dev/mapper/cleanit").unwrap();
    let _ = deactivate(path, "cleanit");

    File::create("/dev/mapper/root").unwrap();
    let path = init("/dev/mapper/root").unwrap();
    let _ = deactivate(path, "root");

    if let Err(e) = wipe_disk(selected_disk) {
        eprintln!("Failed to wipe disk '{}': {}", selected_disk, e);
        return;
    }

    let size = SECTOR_SIZE.lock().unwrap();
    create_partitions(selected_disk, *size);

    println!(
        "IS_NVME: {:?}, IS_SSD: {:?}",
        IS_NVME.lock().unwrap(),
        IS_SSD.lock().unwrap()
    );

    loop {
        match create_luks2_container(selected_disk) {
            Ok(_) => println!("LUKS2 container successfully created; disk formatting complete!\n"),
            Err(e) => eprintln!("Failed to create LUKS2 container: {:?}", e),
        };

        if !*WRONG_PASSWORD.lock().unwrap() {
            break;
        }
    }
}
