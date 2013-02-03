#include <iostream>
#include <unistd.h>

#include "Kinect.h"
#include "Math.h"

using namespace std;

class Events : public IKinectEventSink
{
  public:
    virtual void detectedNewUser(int uid) {}
    virtual void addUser(int uid) {}
    virtual void removeUser(int uid) {}

    virtual void setPosition(int uid, float x, float y, float z) {
        //cout << x << ", " << y << ", " << z << endl;
        Vec3T sensor(x, z, y);
        sensor = Matrix44T::scale(1/25.4) * sensor;
        TransformT tr(-35, 0, -30, 0, 0, 73);
        cout << tr.matrix() * sensor << endl;
    }
};

int main(int argc, char **argv)
{
    Events evt;
    Kinect kinect(&evt);
    
    cout << "Waiting for user." << endl;
    kinect.loop();

    return 0;
}
