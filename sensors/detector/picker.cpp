#include "Kinect.h"
#include <unistd.h>

int main(int argc, char **argv)
{
	Network link("Temp");
	Kinect kinect(&link);
	alarm(20);
	kinect.loop();
	return 0;
}
