#include "typedefs.hh"
#include <boost/regex.hpp>
#include <cstdio>
#include <string>
#include <scn/scan.h>
#include <print>
#include <sys/swap.h>
#include <sstream>
#include <unistd.h>
import djb2a;

std::string o_partition;
std::string partition;
std::stringstream ss;
long int total_ram;

void select_disk()
{
	if (system("clear")) {
		perror("Failed to clear terminal");
		exit(1);
	}

	if (system("lsblk -o PATH,MODEL,PARTLABEL,FSTYPE,FSVER,SIZE,FSUSE%,FSAVAIL,MOUNTPOINTS")) {
		perror("Failed to retrieve disk information");
		exit(1);
	}

	const auto disk_input = scn::prompt<std::string>(
		"\nExample disks: /dev/sda, /dev/nvme0n1.\nInput your desired disk, then press ENTER: ",
		"{}");
	const auto &[di_output] = disk_input->values();

	boost::regex ssd("/dev/sd[a-z]");
	boost::regex nvme("/dev/(nvme|mmc)([0-9])n1");

	if (boost::regex_match(di_output, ssd) ||
	    boost::regex_match(di_output, nvme)) {
		std::println("Disk selected -> {}", di_output);

		const auto prompt =
			scn::prompt<std::string>("Are you sure [Y/N]: ", "{}");
		const auto &[prompt_output] = prompt->values();

		switch (hash_djb2a(prompt_output)) {
			// clang-format off
			case "y"_sh:
				break;
			// clang-format off
			case "Y"_sh:
				break;
			default:
				select_disk();
		}
	} else {
		select_disk();
	}

	o_partition = di_output;
}

void remove_partitions()
{
	// Ensure swap isn't used, otherwise this partition cannot be deleted
	partition = o_partition;
	partition.append("*");

	ss << "swapoff " << partition;

	if (system(ss.str().c_str())) {
		perror("swapoff status");
	}

	partition = o_partition;
	// ss.clear() is unnecessary, but just in case...
	ss.str(""); ss.clear();
	ss << "wipefs -af " << partition;
	if(system(ss.str().c_str())) {
		perror("wipefs status");
	}

	ss.str(""); ss.clear();
	ss << "sgdisk -Z " << partition;
	if(system(ss.str().c_str())) {
		perror("sgdisk status");
	}
}

void wipe_disk_prompt() {
	const auto prompt = scn::prompt<std::string>("\n\nWith 'Secure' the estimated wait time is minutes up to hours, depending on both the disk's type and size.\nType either Secure or Normal: ", "{}");
	const auto&[prompt_output] = prompt->values();

	switch (hash_djb2a(prompt_output))
	{
		// clang-format off
		case "Secure"_sh:
			remove_partitions();
			ss.str(""); ss.clear();
			ss << "cryptsetup open --type plain -d /dev/urandom  " << partition << " cleanit";
			if(system(ss.str().c_str()))
			{
				perror("cryptsetup status");
			}
			if(system("ddrescue --force /dev/zero /dev/mapper/cleanit"))
			{
				perror("ddrescue status");
			}
			if(system("cryptsetup close cleanit"))
			{
				perror("cryptsetup status");
			}
			break;
		// clang-format off
		case "Normal"_sh:
			remove_partitions();
			break;
		default:
			wipe_disk_prompt();
	}
	
}

void create_partitions(){
	total_ram = (sysconf(_SC_PHYS_PAGES)) * (sysconf(_SC_PAGE_SIZE)) / (1024 * 1024);

	ss.str(""); ss.clear();
	ss << "sgdisk -a 2048 -o " << partition;
	if(system(ss.str().c_str())) {
		perror("sgdisk status");
	}

	ss.str(""); ss.clear();
	ss << "sgdisk -n 1::+1024M --typecode=1:ef00 --change-name=1:'BOOTEFI' " << partition;
	if(system(ss.str().c_str())) {
		perror("sgdisk status");
	}

	ss.str(""); ss.clear();
	ss << "sgdisk -n 2::+" << total_ram << "M --typecode=2:8200 " << partition;
	if(system(ss.str().c_str())) {
		perror("sgdisk status");
	}

	ss.str(""); ss.clear();
	ss << "sgdisk -n 3::-0 --typecode=3:8300 --change-name=3:'ROOT' " << partition;
	if(system(ss.str().c_str())) {
		perror("sgdisk status");
	}
	
	// Make the Linux kernel use the latest partition tables without rebooting
	ss.str(""); ss.clear();
	ss << "partprobe " << partition;
	if(system(ss.str().c_str())) {
		perror("partprobe status");
	}

	ss.str(""); ss.clear();
	ss << "mkfs.fat -F 32 " << partition << "1";
	if(system(ss.str().c_str())) {
		perror("mkfs.fat status");
	}

	ss.str(""); ss.clear();
	ss << "mkswap " << partition << "2";
	if(system(ss.str().c_str())) {
		perror("mkswap status");
	}
}

luks_setup() {
	
}

int main()
{
	select_disk();
	wipe_disk_prompt();
	create_partitions();
	luks_setup();
	return 0;
}