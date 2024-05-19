use std::fs::File;
use cryptsetup_rs::device::{deactivate, init};
use regex::Regex;
mod funcs;

fn main() {
    let mut wrong_option = false;
    let mut wrong_disk = false;
    loop {
        disk_selection(&mut wrong_option, &mut wrong_disk);
        if !wrong_disk && !wrong_disk {
            break;
        }
    }

    disk_editing();
}

fn disk_selection(wrong_option: &mut bool, wrong_disk: &mut bool) {
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

    if ssd.is_some() || nvme.is_some() {
        println!("\n{:?} {:?}\n", ssd, nvme);
    } else {
        *wrong_disk = true;
        return;
    }

    let input = funcs::prompt("Are you sure [y/n]: ");

    match input.to_lowercase().as_ref() {
        "y" if input.len() == 1 => println!("Yes"),
        "n" if input.len() == 1 => disk_selection(wrong_option, wrong_disk),
        _ => {
            *wrong_option = true;
            return;
        }
    }
}

fn disk_editing() {
    // Close these two LUKS containers if opened prior.
    File::create("/dev/mapper/cleanit").unwrap();
    let path = init("/dev/mapper/cleanit").unwrap();
    let _ = deactivate(path, "cleanit");
    
    File::create("/dev/mapper/root").unwrap();
    let path = init("/dev/mapper/root").unwrap();
    let _ = deactivate(path, "root");
}
