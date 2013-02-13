#include <iostream>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <utility>
#include <vector>

#include "Math.h"

using namespace std;

class Timer
{
    struct timeval mStart;
    double mDuration;

  public:
    Timer(bool autostart = true) : mDuration(0) { if (autostart) start(); }
    void start() { gettimeofday(&mStart, NULL); }
    double stop() {
        struct timeval end;
        gettimeofday(&end, NULL);
        struct timeval diff;
        timersub(&end, &mStart, &diff);
        mDuration = diff.tv_sec + (diff.tv_usec / 1000000.0);
        return mDuration;
    }
    double duration() const { return mDuration; }
};

typedef pair<Vec3T,Vec3T> PointMatch;
typedef vector<PointMatch> PointsVector;
typedef PointsVector::iterator PointsIter;

int
ReadPoints(Vec3T &sensor, Vec3T &reference, PointsVector &points)
{
    cout << "Enter the sensor location in room coordinates \"x y z\"" << endl;
    double x, y, z;
    cin >> x >> y >> z;
    sensor.set(x, y, z);
    cout << "Enter the measurement center in room coordinates \"x y z\"" << endl;
    cin >> x >> y >> z;
    reference.set(x, y, z);
    cout << "Enter point pairs input to output \"sX sY sZ mX mY mZ\"" << endl;
    bool ok = true;
    do {
        double sX, sY, sZ, mX, mY, mZ;
        ok = cin >> sX >> sY >> sZ >> mX >> mY >> mZ;
        if (ok)
            points.push_back(PointMatch(Vec3T(sX,sY,sZ), Vec3T(mX,mY,mZ)));
    } while(ok);
}

const Number METERS_PER_FOOT = 0.305;
const Number METERS_PER_INCH = METERS_PER_FOOT / 12;

int
main(int argc, char **argv)
{
    Vec3T sensor;
    Vec3T reference;
    PointsVector points;
    ReadPoints(sensor, reference, points);

    PointsVector raw(points.size());
    copy(points.begin(), points.end(), raw.begin());

    cout << "Sensor at: " << sensor << endl;
    cout << "Measurement reference at: " << reference << endl;

    // Remap the kinect coordinates into the same coordinate type and
    // orientation of the room.
    for (PointsIter iter = points.begin(); iter != points.end(); ++iter) {
        // Kinect's axes are reported in mm relative to:
        // -> x+
        //  /\ y+
        // out z+
        iter->first = Matrix44T::flipYZ() * Matrix44T::scale(1/25.4) * iter->first;
    }

    // Recenter the points measured position to be relative to the sensor. This
    // lets us leave the transform out of the matrix we are computing, which
    // saves a few cycles and makes everything easier to debug.
    Vec3T sensorToReference = reference - sensor;
    for (PointsIter iter = points.begin(); iter != points.end(); ++iter) {
        iter->second = iter->second + sensorToReference;
    }

    double minError = 999999999.;
    TransformT bestTrans;
    LinSpaceT rRotX(-180, 179, 360);
    LinSpaceT rRotY(-180, 179, 180);
    LinSpaceT rRotZ(-180, 179, 360);
    LinSpaceT rX(-0, 0, 1);
    LinSpaceT rY(-0, 0, 1);
    LinSpaceT rZ(-0, 0, 1);
    for (rRotX.begin(); !rRotX.done(); rRotX.next())
        for (rRotY.begin(); !rRotY.done(); rRotY.next())
            for (rRotZ.begin(); !rRotZ.done(); rRotZ.next())
                for (rX.begin(); !rX.done(); rX.next())
                    for (rY.begin(); !rY.done(); rY.next())
                        for (rZ.begin(); !rZ.done(); rZ.next()) {
                            // The trial transform and matrix.
                            TransformT tr(rRotX.v(),
                                          rRotY.v(),
                                          rRotZ.v(),
                                          rX.v(),
                                          rY.v(),
                                          rZ.v());
                            Matrix44T M = tr.matrix();

                            // Get the average error of all points.
                            double error = 0;
                            size_t i = 0;
                            for (PointsIter iter = points.begin(); iter != points.end(); ++iter, ++i) {
                                Vec3T &kinect = iter->first;
                                Vec3T &room = iter->second;

                                Vec3T transformed = M * kinect;
                                error += (room - transformed).length();
                            }
                            error /= points.size();

                            // Minify the average error.
                            if (error < minError) {
                                minError = error;
                                bestTrans = tr;
                            }
                        }

    cout << "Best Transform: " << bestTrans << endl;
    Number avgErr = 0;
    for (PointsIter iter = points.begin(); iter != points.end(); ++iter) {
        Vec3T &kinect = iter->first;
        Vec3T &room = iter->second;
        Vec3T transformed = bestTrans.matrix() * kinect;
        double error = (room - transformed).length();
        avgErr += error;
        cout << "Error: " << error << " with " << room << " -> " << transformed << endl;
    }
    cout << "ERROR: " << (avgErr / points.size()) << endl;

    // Get actual transform from sensor -> room wrt sensor -> room wrt base.
    Matrix44T M = Matrix44T::scale(METERS_PER_INCH) *
                  Matrix44T::translate(sensor) *
                  bestTrans.matrix() *
                  Matrix44T::scale(1/25.4) *
                  Matrix44T::flipYZ();

    // Multiply all points to check our work.
    for (PointsIter iter = raw.begin(); iter != raw.end(); ++iter) {
        Vec3T &sensor = iter->first;
        Vec3T &measured = iter->second;
        Vec3T room = (measured + reference) * METERS_PER_INCH;
        Number error = (room - (M * sensor)).length();
        cout << "err: " << error << " : " << room << " -> " << (M * sensor) << endl;
    }

    cerr << M.serialize() << endl;
}
