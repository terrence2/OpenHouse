#include <iostream>
#include <string>
#include <XnStatus.h>
#include "Kinect.h"

using namespace std;

/* static */ void XN_CALLBACK_TYPE
Kinect::onNewUser(xn::UserGenerator &user, XnUserID nId, void *cookie)
{
    cout << "New User: " << nId << endl;
    user.GetSkeletonCap().RequestCalibration(nId, TRUE);

    Kinect *self = static_cast<Kinect*>(cookie);
    /* Note: we send the new-user event /after/ "calibration" completes not before. */
}

/* static */ void XN_CALLBACK_TYPE
Kinect::onLostUser(xn::UserGenerator &user, XnUserID nId, void *cookie)
{
    cout << "Lost User: " << nId << endl;

    Kinect *self = static_cast<Kinect*>(cookie);
    self->mSink->removeUser(nId);
}

/* static */ void XN_CALLBACK_TYPE
Kinect::onStartCalibration(xn::SkeletonCapability&, XnUserID nId, void *cookie)
{
    cout << "Start Calibration: " << nId << endl;

    Kinect *self = static_cast<Kinect*>(cookie);
    self->mSink->detectedNewUser(nId);
}

/* static */ void XN_CALLBACK_TYPE
Kinect::onCompleteCalibration(xn::SkeletonCapability&, XnUserID nId, XnCalibrationStatus eStatus, void* cookie)
{
    Kinect *self = static_cast<Kinect*>(cookie);
    if (eStatus == XN_CALIBRATION_STATUS_OK) {
        cout << "Calibration Complete: " << nId << endl;
        self->mUser.GetSkeletonCap().StartTracking(nId);
        self->mSink->addUser(nId);
        return;
    }

    cout << "Calibration Failed: " << nId << " - ";
    self->mSink->removeUser(nId);
}


KinectError::KinectError(XnStatus aRetCode, const char* aMsg)
{
    mMsg = string(aMsg) + ": " + string(xnGetStatusString(aRetCode));
}

Kinect::Kinect(IKinectEventSink *sink)
{
    mSink = sink;

    XnStatus ok;
    xn::EnumerationErrors errors;
    
    // Initialize context object
    ok = mContext.InitFromXmlFile("config.xml", mScriptNode, &errors);
    if (ok == XN_STATUS_NO_NODE_PRESENT) {
        XnChar str[1024];
        errors.ToString(str, 1024);
        cerr << "Enumeration failed, errors are:" << endl;
        cerr << str << endl;
    }
    if (ok)
        throw KinectError(ok, "context init");

    // Find the DepthGenerator node.
    ok = mContext.FindExistingNode(XN_NODE_TYPE_DEPTH, mDepth);
    if (ok)
        throw KinectError(ok, "missing depth node");

    // Find the user node.
    ok = mContext.FindExistingNode(XN_NODE_TYPE_USER, mUser);
    if (ok)
        throw KinectError(ok, "missing user node");

    // Verify that the user node is adequate to our purposes.
    if (!mUser.IsCapabilitySupported(XN_CAPABILITY_SKELETON))
        throw KinectError(ok, "no support for skeltons");
    if (mUser.GetSkeletonCap().NeedPoseForCalibration())
        throw KinectError(ok, "would need to pose for calibration");

    // Connect to user events.
    XnCallbackHandle hUser, hCalibStart, hCalibComplete;
    ok = mUser.RegisterUserCallbacks(onNewUser, onLostUser, (void*)this, hUser);
    if (ok)
        throw KinectError(ok, "failed to register user callbacks");
    ok = mUser.GetSkeletonCap().RegisterToCalibrationStart(onStartCalibration, (void*)this, hCalibStart);
    if (ok)
        throw KinectError(ok, "failed to register calibration start callback.");
    ok = mUser.GetSkeletonCap().RegisterToCalibrationComplete(onCompleteCalibration, this, hCalibComplete);
    if (ok)
        throw KinectError(ok, "failed to register calibration start callback.");

    // Ensure that joints are available.
    mUser.GetSkeletonCap().SetSkeletonProfile(XN_SKEL_PROFILE_ALL);

    // Go!
    ok = mContext.StartGeneratingAll();
    if (ok)
        throw KinectError(ok, "start generating");
}

Kinect::~Kinect()
{
    mScriptNode.Release();
    mDepth.Release();
    mUser.Release();
    mContext.Release();
}

void
Kinect::loop()
{
    while (!xnOSWasKeyboardHit()) {
        mContext.WaitOneUpdateAll(mUser);
        
        XnUserID userBuf[1024];
        XnUInt16 numUsers = 1024;
        mUser.GetUsers(userBuf, numUsers);
        for (XnUInt16 i = 0; i < numUsers; ++i) {
            if (!mUser.GetSkeletonCap().IsTracking(userBuf[i]))
                continue;

            XnSkeletonJointTransformation torso;
            mUser.GetSkeletonCap().GetSkeletonJoint(userBuf[i], XN_SKEL_TORSO, torso);
            cout << /*"User (" << userBuf[i] << ") torso: " <<*/
                         torso.position.position.X << " " <<
                         torso.position.position.Y << " " <<
                         torso.position.position.Z << endl;

            mSink->setPosition(userBuf[i], torso.position.position.X,
                                           torso.position.position.Y,
                                           torso.position.position.Z);
        }
    }
}


