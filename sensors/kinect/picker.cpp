/* This Source Code Form is subject to the terms of the GNU General Public
 * License, version 3. If a copy of the GPL was not distributed with this file,
 * You can obtain one at https://www.gnu.org/licenses/gpl.txt. */
#include <iostream>
#include <unistd.h>

#include "Kinect.h"
#include "Math.h"

using namespace std;

class Events : public IKinectEventSink
{
  public:
    int uid_;
    Vec3T lastPosition;

    Events() : uid_(-1) {}

    virtual void detectedNewUser(int uid) {}

    virtual void addUser(int uid) {}

    virtual void removeUser(int uid) {
        // Reset our tracked uid so a new user can track.
        if (uid_ == uid) {
            cout << "Lost user: " << uid << endl;
            uid_ = -1;
        }
    }

    virtual void setPosition(int uid, float x, float y, float z) {
        // Accept new user if we aren't tracking one.
        if (uid_ == -1) {
            cout << "Setting user to: " << uid << endl;
            uid_ = uid;
        }

        // Update our most recent position.
        lastPosition.set(x, y, z);

        cout << x << ", " << y << ", " << z << endl;
    }
};

int main(int argc, char **argv)
{
    Events evt;
    Kinect kinect(&evt);
    
    cout << "Waiting for user." << endl;
    kinect.loop();

    cerr << evt.lastPosition << endl;

    return 0;
}
