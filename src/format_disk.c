#include "typedefs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <stdbool.h>

bool rp_continue;

void remove_partitions(void)
{
}

void select_partitions(void)
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

	char disk_name[INT8_MAX];
	char prompt[8];
	const char *invalid = "!@#$%^&*()_+=-\\[].;'";

	if (fgets(disk_name, sizeof(disk_name), stdin)) {
		size_t length = strcspn(disk_name, invalid);

		if (length != strlen(disk_name)) {
			puts("\nDisk has been entered incorrectly, try again.\n");
			// There's an issue where prior incorrect disks show up in the selected disk dialog.
			// TODO: fix this later to be able to loop back to the select_partitions() function.
			exit(1);
		}

		printf("\nSelected disk: %s\nPress y then ENTER to confirm, n to deny.\n",
		       disk_name);

		if (fgets(prompt, sizeof(prompt), stdin) == NULL) {
			perror("Failed to read stream");
			exit(1);
		}

		switch (prompt[0]) {
		case 121: // y
			rp_continue = 1;
			break;
		case 89: // Y
			rp_continue = 1;
			break;
		default:
			select_partitions();
			break;
		}
	}
}

int main(void)
{
	select_partitions();
	if (rp_continue) {
		printf("Continue!\n");
	}
	return 0;
}