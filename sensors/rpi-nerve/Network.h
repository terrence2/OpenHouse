/* This Source Code Form is subject to the terms of the GNU General Public
 * License, version 3. If a copy of the GPL was not distributed with this file,
 * You can obtain one at https://www.gnu.org/licenses/gpl.txt. */
#ifndef Network_h__
#define Network_h__

#include <string>

#include <zmq.hpp>

class Network
{
  public:
    static const int SensorPort = 31975;
    static const int ControlPort = 31976;

    Network(const std::string &name);

    void detectedMovement(bool state);
    void updateTempAndHumidity(float temp, float humidity);

    void checkControlSock();

  protected:
    const std::string mName;
    zmq::context_t mCtx;
    zmq::socket_t mSensorSock;
    zmq::socket_t mControlSock;
};

#endif // Network_h__
