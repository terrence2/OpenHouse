// OpenNI includes
#include <libfreenect.h>
#include <libfreenect_sync.h>

// Standard includes
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>


int main(int argc, char **argv)
{
	if (freenect_sync_set_led(LED_GREEN, 0)) {
		fprintf(stderr, "Unable to set LED, guessing no kinect.\n");
		return 6;
	}

	if (argc < 2) {
		fprintf(stderr, "No tilt angle (-30, 30) specified.\n");
		return 1;
	}

	int angle = atol(argv[1]);
	if (angle < -30 || angle > 30) {
		fprintf(stderr, "Angle %d is out of range (-30, 30).\n", angle);
		return 2;
	}

	if (freenect_sync_set_tilt_degs(angle, 0)) {
		fprintf(stderr, "Set angle failed!\n");
		freenect_sync_set_led(LED_OFF, 0);
		return 3;
	}

	// Get the raw accelerometer values and tilt data
	freenect_raw_tilt_state *state = NULL;
	if (freenect_sync_get_tilt_state(&state, 0)) {
		fprintf(stderr, "Failed to get tilt state.\n");
		freenect_sync_set_led(LED_OFF, 0);
		return 4;
	}

	printf("New state: %p\n", state);
	freenect_sync_set_led(LED_OFF, 0);

	return 0;
}
