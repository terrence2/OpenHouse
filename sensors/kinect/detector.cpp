#include <iostream>
#include <memory>
#include "Network.h"
#include "Kinect.h"

using namespace std;

static void run(const string &name, const string &controller)
{
	Network link(name);
	Kinect kinect(&link);
	
	cout << "Started" << endl;
	kinect.loop();
	cout << "Finished" << endl;
}

int main(int argc, char **argv)
{
	if (argc <= 1) {
		cout << "Arg1 must be NAME" << endl;
		return 1;
	}
	string name(argv[1]);

	string controller(DefaultController);
	if (argc > 2)
		controller = string(argv[2]);

	try {
		run(name, controller);
	} catch(KinectError e) {
		cerr << "KinctError- " << e.message() << endl;
		return 1;
	}
	return 0;
}
