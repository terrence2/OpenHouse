#ifndef _Kinect_h__
#define _Kinect_h__

#include <exception>
#include <XnCppWrapper.h>
#include "Network.h"

class Kinect
{
  public:
	Kinect(Network *link);
	~Kinect();

	void loop();
	static void XN_CALLBACK_TYPE onNewUser(xn::UserGenerator &user, XnUserID nId, void *cookie);
	static void XN_CALLBACK_TYPE onLostUser(xn::UserGenerator &user, XnUserID nId, void *cookie);
	static void XN_CALLBACK_TYPE onStartCalibration(xn::SkeletonCapability&, XnUserID nId, void *cookie);
	static void XN_CALLBACK_TYPE onCompleteCalibration(xn::SkeletonCapability&, XnUserID nId, 
														XnCalibrationStatus eStatus, void* cookie);

  protected:
	xn::Context mContext;
	xn::ScriptNode mScriptNode;
	xn::DepthGenerator mDepth;
	xn::UserGenerator mUser;
	Network *mLink;
};

class KinectError
{
  public:
	KinectError(XnStatus aRetCode, const char* aMsg);

	const std::string& message() const { return mMsg; }

  private:
	std::string mMsg;
};

#endif // _Kinect_h__
