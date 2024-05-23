use std::io::{self, Write};
use std::process::Command;
use nix::libc;

pub fn prompt(description: &str) -> String {
    print!("{description}");

    io::stdout().flush().expect("Failed to flush stdout");

    let mut s = String::new();
    io::stdin().read_line(&mut s).expect("Failed to read line");

    s.trim().to_string()
}

pub fn terminal(description: &str) {
    let output = Command::new("sh")
        .arg("-c")
        .arg(description)
        .output()
        .expect("Failed to execute");

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
