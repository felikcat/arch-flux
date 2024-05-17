#include "typedefs.hh"
#include <boost/regex.hpp>
#include <iostream>
#include <string_view>
#include <string>
import djb2a;

bool di_continue;

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
		printf("Disk selected: %s\nAre you sure: ", disk_input.c_str());
	} else if (boost::regex_search(disk_input, match, nvme)) {
		printf("Disk selected: %s\nAre you sure: ", disk_input.c_str());
	}

	std::string prompt;
	std::getline(std::cin.ignore(), prompt);

	switch (hash_djb2a(prompt)) {
	case "y"_sh:
		di_continue = 1; 
		break;
	case "Y"_sh:
		di_continue = 1;
		break;
	default:
		select_partitions();
		break;
	}
}

int main()
{
	select_partitions();
	if (di_continue) {
		printf("Continue!\n");
	}
	return 0;
}