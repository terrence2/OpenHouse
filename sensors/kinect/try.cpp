#include <iostream>
#include <unistd.h>

#include <tclap/CmdLine.h>

#include "Kinect.h"
#include "Math.h"

using namespace std;

class Events : public IKinectEventSink
{
    Matrix44T M;

  public:
    Events(const vector<Number> &registration) {
        size_t pos = 0;
        for (size_t i = 0; i < 4; ++i)
            for (size_t j = 0; j < 4; ++j)
                M.set(i, j, registration[pos++]);
    }

    virtual void detectedNewUser(int) {}
    virtual void addUser(int) {}
    virtual void removeUser(int) {}

    virtual void setPosition(int, float x, float y, float z) {
        cout << M * Vec3T(x, y, z) << endl;
    }
};

int main(int argc, char **argv)
{
    TCLAP::CmdLine cmd("See if a particular matrix works well.", ' ', "0.0.0");
    TCLAP::UnlabeledMultiArg<Number> registrationArg("registration",
                    "A 16 element vector of matrix elements.", true, "VEC", false);
    cmd.add(registrationArg);
    cmd.parse(argc, argv);

    vector<Number> registration = registrationArg.getValue();
    if (registration.size() != 16) {
        cerr << "The registration vector must be 16 long." << endl;
        return 1;
    }

    Events evt(registration);
    Kinect kinect(&evt);

    cout << "Waiting for user." << endl;
    kinect.loop();

    return 0;
}
