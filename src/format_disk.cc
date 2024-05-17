#include "typedefs.h"
#include <boost/regex.hpp>
#include <iostream>

bool rp_continue;

bool validate_disk_name(const std::string& dn)
{
	static const boost::regex dnr("/dev/sd[0-9,a-z]");
	return regex_match(dn, dnr);
}

void remove_partitions()
{
}

void select_partitions()
{
	if (system("clear")) {
		perror("Failed to clear terminal");
		exit(1);
	}

	if (system("lsblk -o PATH,MODEL,PARTLABEL,FSTYPE,FSVER,SIZE,FSUSE%,FSAVAIL,MOUNTPOINTS")) {
		perror("Failed to retrieve disk information");
		exit(1);
	}

	puts("\nDisk examples: /dev/sda or /dev/nvme0n1. Don\'t use partition numbers like: /dev/sda1 or /dev/nvme0n1p1.\nInput your desired disk, then press ENTER: ");

	std::string disk_input;
	std::cin >> disk_input;
	boost::regex ssd("/dev/sd[a-z]");
	boost::regex nvme("/dev/(nvme|mmc)([0-9])n1");
	boost::smatch match;

	if (boost::regex_search(disk_input, match, ssd)) {
		printf("This is %s", disk_input.c_str());
	}
	else if (boost::regex_search(disk_input, match, nvme)) {
		printf("This is %s", disk_input.c_str());
	}
}

int main()
{
	select_partitions();
	if (rp_continue) {
		printf("Continue!\n");
	}
	return 0;
}