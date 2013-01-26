#ifndef _Kinect_h__
#define _Kinect_h__

#include <exception>
#include <string>
#include <XnCppWrapper.h>

class IKinectEventSink
{
  public:
	virtual void detectedNewUser(int uid) = 0;
	virtual void addUser(int uid) = 0;
	virtual void removeUser(int uid) = 0;
    virtual void setPosition(int uid, float x, float y, float z) = 0;
};

class Kinect
{
  public:
    Kinect(IKinectEventSink *sink);
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
    IKinectEventSink *mSink;
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
