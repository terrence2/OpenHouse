#ifndef _Network_h__
#define _Network_h__

#include <exception>
#include <string>

#include <zmq.hpp>

#include "Kinect.h"

static const char *DefaultController = "gorilla";
static const int SensorPort = 31975;
static const int ControlPort = 31976;

class Network : public IKinectEventSink
{
  public:
	Network(const std::string &name);

	void detectedNewUser(int uid);
	void addUser(int uid);
	void removeUser(int uid);
	void setPosition(int uid, float x, float y, float z);

	void checkControlSock();

  protected:
	std::string mName;
	zmq::context_t mCtx;
	zmq::socket_t mSensorSock;
	zmq::socket_t mControlSock;
};

#endif // _Network_h__
