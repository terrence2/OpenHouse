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
Network::detectedNewUser(int uid)
{
	string data = "{\"name\": \"" + mName + "\", \"type\": \"MAYBEADDUSER\", \"uid\": " +
		to_string(uid) + "}";
	SendMessage(mSensorSock, data);
}

void
Network::addUser(int uid)
{
	string data = "{\"name\": \"" + mName + "\", \"type\": \"ADDUSER\", \"uid\": " +
		to_string(uid) + "}";
	SendMessage(mSensorSock, data);
}

void
Network::removeUser(int uid)
{
	string data = "{\"name\": \"" + mName + "\", \"type\": \"REMOVEUSER\", \"uid\": " + 
		to_string(uid) + "}";
	SendMessage(mSensorSock, data);
}

void
Network::setPosition(int uid, float x, float y, float z)
{
	string data = "{\"name\": \"" + mName + "\", \"type\": \"POSITION\", \"uid\": " + 
		to_string(uid) +
			", \"X\": " + to_string(x) + 
			", \"Y\": " + to_string(y) + 
			", \"Z\": " + to_string(z) + "}";
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
