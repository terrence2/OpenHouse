#include <assert.h>
#include <string>
#include <iostream>
#include <zmq.hpp>
#include "Network.h"

using namespace std;
using namespace zmq;

Network::Network(const std::string &name)
  : mName(name),
    mCtx(2),
    mSensorSock(mCtx, ZMQ_PUB),
    mControlSock(mCtx, ZMQ_REP)
{
    mSensorSock.bind(("tcp://*:" + to_string(SensorPort)).c_str());
    mControlSock.bind(("tcp://*:" + to_string(ControlPort)).c_str());
}

static void
SendMessage(socket_t &sock, const string &data)
{
    message_t msg(data.length());
    memcpy(msg.data(), data.c_str(), data.length());
    sock.send(msg);
}

void
Network::detectedMovement(bool state)
{
    string s = state ? "true" : "false";
    string data = "{\"name\": \"" + mName + "\", \"type\": \"MOVEMENT\", \"state\": " + s + " }";
    SendMessage(mSensorSock, data);
}

void
Network::updateTempAndHumidity(float temp, float humidity)
{
    string data = "{\"name\": \"" + mName + "\", \"type\": \"TEMP_HUMIDITY\""
            ", \"temp\": " + to_string(temp) +
            ", \"humidity\": " + to_string(humidity) +
            "}";
    SendMessage(mSensorSock, data);
}

void
Network::checkControlSock()
{
    message_t msg;
    bool rv = mControlSock.recv(&msg, ZMQ_NOBLOCK);
    if (!rv)
        return;

    cout << "Got Control message!" << endl;
}
