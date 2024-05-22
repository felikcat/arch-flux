use cryptsetup_rs::device::{deactivate, init};
use libparted_sys::*;
use nix::libc::{self};
use nix::unistd::close;
use nix::{self, ioctl_read_bad};
use regex::Regex;
use std::ffi::CString;
use std::fmt::{Display, Formatter, Result};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::sync::Mutex;

#[cfg(any(unix, target_os = "wasi"))]
use std::os::fd::{AsRawFd, RawFd};

mod funcs;

static SECTOR_SIZE: Mutex<i32> = Mutex::new(0);

fn main() {
    let mut wrong_option = false;
    let mut wrong_disk = false;
    let mut selected_disk = "/dev/null".to_string();

    loop {
        disk_selection(&mut wrong_option, &mut wrong_disk, &mut selected_disk);
        if !wrong_disk && !wrong_disk {
            break;
        }
    }

    disk_editing(&mut selected_disk);
}

fn disk_selection(wrong_option: &mut bool, wrong_disk: &mut bool, selected_disk: &mut String) {
    // Clear terminal
    print!("{esc}c", esc = 27 as char);

    if *wrong_option {
        println!("NOTICE: Please enter 'y' or 'n'.\n");
    }
    if *wrong_disk {
        println!("NOTICE: An invalid disk has been selected, try again.\n")
    }

    *wrong_option = false;
    *wrong_disk = false;

    funcs::terminal("lsblk -o PATH,MODEL,PARTLABEL,FSTYPE,FSVER,SIZE,FSUSE%,FSAVAIL,MOUNTPOINTS");

    let input = funcs::prompt("\nExample disks: /dev/sda, /dev/nvme0n1.\nInput your desired disk, then press ENTER: ");

    let ssd = Regex::new(r"/dev/sd[a-z]").unwrap().find(&input);
    let nvme = Regex::new(r"/dev/(nvme|mmc)([0-9])n1").unwrap().find(&input);

    struct RegexMatch<'a>(&'a str); // Store

    impl<'a> Display for RegexMatch<'a> {
        fn fmt(&self, format: &mut Formatter) -> Result {
            write!(format, "{}", self.0)
        }
    }

    match ssd.or(nvme) {
        Some(match_str) => {
            let regex_match = RegexMatch(match_str.as_str());
            println!("\nSelected disk: {}\n", regex_match);
            *selected_disk = regex_match.to_string();
        }
        None => {
            *wrong_disk = true;
            return;
        }
    }

    let input = funcs::prompt("Are you sure [y/n]: ");

    match input.to_lowercase().as_ref() {
        "y" if input.len() == 1 => return,
        "n" if input.len() == 1 => disk_selection(wrong_option, wrong_disk, selected_disk),
        _ => {
            *wrong_option = true;
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

    let mut size_lock = SECTOR_SIZE.lock().unwrap();
    *size_lock = size;

    let buffer_size =
        usize::try_from(size).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid sector size"))?;
    let buffer = vec![0u8; buffer_size];

    device.write_all(&buffer)?; // Write zeros to disk.
    device.sync_all()?; // Ensure writes are flushed to disk.

    close(fd).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to close disk device"))?;

    println!("Wiped disk '{}' successfully", device_path);

    Ok(())
}

// https://www.gnu.org/software/parted/api/modules.html
fn create_partitions(device_path: &str, sector_size: i32) {
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
        println!("sector_size: {}, end: {}", sector_size, end);
    }
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
}
