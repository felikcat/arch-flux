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
		printf("Failed to retrieve disk information.");
		exit(1);
	}

	puts("\nDisk examples: /dev/sda or /dev/nvme0n1. Don\'t use partition numbers like: /dev/sda1 or /dev/nvme0n1p1.\nInput your desired disk, then press ENTER: ");

	char disk_name[INT8_MAX];
	char prompt[8];

	if (fgets(disk_name, INT8_MAX, stdin)) {
		disk_name[strcspn(disk_name, (" "))] = 0;
		printf("\nSelected disk: %s\nPress y then ENTER to confirm, n to deny.\n",
		       disk_name);
		i32 length = sizeof(prompt);

		if (fgets(prompt, length, stdin) == NULL) {
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