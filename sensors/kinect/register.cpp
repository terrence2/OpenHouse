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
ReadPoints(Vec3T &center, PointsVector &points)
{
    double a, b, c, x, y, z;
    cout << "Enter the center coordinate \"x y z\"" << endl;
    cin >> a >> b >> c;
    center.set(a, b, c);
    cout << "Enter point pairs input to output \"a b c x y z\"" << endl;
    bool ok = true;
    do {
        ok = cin >> a >> b >> c >> x >> y >> z;
        if (ok)
            points.push_back(PointMatch(Vec3T(a,b,c), Vec3T(x,y,z)));
    } while(ok);
}

int
main(int argc, char **argv)
{
    Vec3T pos;
    PointsVector points;
    ReadPoints(pos, points);

    // Remap to inches.
    for (PointsIter iter = points.begin(); iter != points.end(); ++iter) {
        /*
        Kinect's axes are reported in mm relative to:
        -> x+
        /\ y+
        out z+
        */
        Vec3T room(-iter->second.get(0),
                   -iter->second.get(1),
                   iter->second.get(2));
        Vec3T kinect(iter->first.get(0),
                     iter->first.get(2),
                     iter->first.get(1));
        cout << "1: " << room << " -> " << kinect << endl;
        kinect = (Matrix44T::scale(1/25.4) * kinect); 
        cout << "2: " << room << " -> " << kinect << endl;
        
        iter->first = kinect;
        //iter->second = room;
    }
    pos = Vec3T(0, 0, 73);

    double minError = 999999999.;
    TransformT bestTrans;
    LinSpaceT rRotX(-180, 179, 360);
    LinSpaceT rRotY(-0, 0, 1);
    LinSpaceT rRotZ(-180, 179, 360);
    LinSpaceT rX(-0, 0, 1);
    LinSpaceT rY(-0, 0, 1);
    //LinSpaceT rZ(-0, 0, 1);
    LinSpaceT rZ(0, 79, 80);
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
    for (PointsIter iter = points.begin(); iter != points.end(); ++iter) {
        Vec3T &kinect = iter->first;
        Vec3T &room = iter->second;
        Vec3T transformed = bestTrans.matrix() * kinect;
        double error = (room - transformed).length();
        cout << "Error: " << error << " with " << room << " -> " << transformed << endl;
    }
    cerr << bestTrans.ang().get(0) << " " <<
            bestTrans.ang().get(1) << " " <<
            bestTrans.ang().get(2) << " " <<
            bestTrans.pos().get(0) << " " <<
            bestTrans.pos().get(1) << " " <<
            bestTrans.pos().get(2) << endl;
}
