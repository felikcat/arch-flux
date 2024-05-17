#include "typedefs.hh"
#include <boost/regex.hpp>
#include <iostream>
#include <string>
#include <scn/scan.h>
#include <print>
import djb2a;

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

	const auto disk_input = scn::prompt<std::string>(
		"\nExample disks: /dev/sda, /dev/nvme0n1.\nInput your desired disk, then press ENTER: ",
		"{}");
	auto [di_output] = disk_input->values();

	boost::regex ssd("/dev/sd[a-z]");
	boost::regex nvme("/dev/(nvme|mmc)([0-9])n1");

	if (boost::regex_match(di_output, ssd) ||
	    boost::regex_match(di_output, nvme)) {
		std::println("Disk selected -> {}", di_output);

		const auto prompt =
			scn::prompt<std::string>("Are you sure: ", "{}");
		const auto &[prompt_output] = prompt->values();

		switch (hash_djb2a(prompt_output)) {
			// clang-format off
				case "y"_sh:
					break;
				// clang-format off
				case "Y"_sh:
					break;
				default:
					select_partitions();
				}
		} else {
			select_partitions();
		}
}

int main()
{
	select_partitions();	
	std::println("Continue!\n");
	return 0;
}