use std::io::{self, BufRead, BufReader, Write};
use std::process::Command;
use libparted_sys::{_PedDevice, _PedDisk, ped_device_destroy, ped_disk_destroy};
use nix::libc;

pub fn prompt(description: &str) -> String {
    print!("{description}");

    io::stdout().flush().expect("Failed to flush stdout");

    let mut s = String::new();
    io::stdin().read_line(&mut s).expect("Failed to read line");

    s.trim().to_string()
}

pub fn prompt_u8(description: &str) -> Vec<u8> {
    print!("{description}");

    io::stdout().flush().expect("Failed to flush stdout");

    let mut buffer = Vec::new();
    let mut reader = BufReader::new(io::stdin());
    
    reader.read_until(b'\n', &mut buffer).expect("Failed to read line");

    if let Some(&b'\n') = buffer.last() {
        buffer.pop();  // Remove newline
        if buffer.last() == Some(&b'\r') {
            buffer.pop();  // Remove carriage return on Windows
        }
    }

    buffer
}


pub fn terminal(description: &str) {
    let output = Command::new("sh")
        .arg("-c")
        .arg(description)
        .output()
        .expect("Failed to execute shell command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{}", stdout);
}

pub fn umount(target: &str, flags: libc::c_int) -> Result<(), String> {
    let target_c = std::ffi::CString::new(target).map_err(|_| "Failed to create CString")?;
    let action = unsafe { libc::umount2(target_c.as_ptr(), flags)};

    if action == 0 {
        Ok(())
    } else {
        Err(format!("Failed to unmount: {}", std::io::Error::last_os_error()))
    }
}

pub fn mib_to_sectors(mib: i64, sector_size: i64) -> i64 {
    let bytes_per_mib: i64 = 1_048_576; // 1024 * 1024
    (mib * bytes_per_mib) / sector_size
}

pub unsafe fn parted_cleanup(disk: *mut _PedDisk, device: *mut _PedDevice) {
    ped_disk_destroy(disk);
    ped_device_destroy(device);
    panic!("{:?}", println!("parted_cleanup was called, panicking..."))
}
