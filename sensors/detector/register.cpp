#include <iostream>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <utility>
#include <vector>

#include "Kinect.h"

#ifdef DEBUG
#define ASSERT(c) if (!(c)) abort();
#else
#define ASSERT(c) ;
#endif

using namespace std;

typedef float Number;

/*
 * An iterator for even subdivision of space.
 */
template <typename T>
class LinSpace
{
	T mStart;
	T mScale;
	T mCurrent;

	size_t mCount;
	size_t mPos;

  public:
	LinSpace(T start, T end, size_t count)
	  : mStart(start), mScale((end - start) / (count - 1)), mCurrent(start), mCount(count), mPos(0) {}
	void begin() { mCurrent = mStart; mPos = 0; }
	bool done() { return mPos >= mCount; }
	void next() {
		mPos += 1;
		mCurrent = mStart + mPos * mScale;
	}
	T v() const { return mCurrent; }
	size_t i() const { return mPos; }
	size_t count() const { return mCount; }
};
typedef LinSpace<Number> LinSpaceT;

template <typename T>
class Vec3
{
	T v[3];

  public:
	Vec3() {}
	Vec3(T x, T y, T z) {
		v[0] = x;
		v[1] = y;
		v[2] = z;
	}

	T get(int i) const {
		ASSERT(i >= 0 && i < 3);
		return v[i];
	}

	void set(int i, T t) {
		ASSERT(i >= 0 && i < 3);
		v[i] = t;
	}

	Vec3 operator-(const Vec3 &other) const {
		Vec3 out(v[0] - other.v[0],
				 v[1] - other.v[1],
				 v[2] - other.v[2]);
		return out;
	}

	double length() const {
		return sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
	}
};
typedef Vec3<Number> Vec3T;

ostream& operator<<(ostream &out, const Vec3T &v)
{
	out << "(" << v.get(0) << ", " << v.get(1) << ", " << v.get(2) << ")";
	return out;
}

template <typename T>
class Matrix44
{
	T v[4][4];

  public:
	Matrix44() {}

	Matrix44(const Matrix44 &other) {
		for (int i = 0; i < 4; ++i)
			for (int j = 0; j < 4; ++j)
				v[i][j] = other.v[i][j];
	}

	void set(int i, int j, T t) {
		ASSERT(i >= 0 && i < 4);
		ASSERT(j >= 0 && j < 4);
		v[i][j] = t;
	}

	Matrix44 operator*(const Matrix44 &other)
	{
		Matrix44 out;
		for (int i = 0; i < 4; ++i) {
			for (int j = 0; j < 4; ++j) {
				T sum = 0;
				for (int k = 0; k < 4; ++k)
					sum += v[i][k] * other.v[k][j];
				out.v[i][j] = sum;
			}
		}
		return out;
	}

	Vec3<T> operator*(const Vec3<T> &vec) {
		Vec3<T> out;
		for (int i = 0; i < 4; ++i) {
			T sum = 0;
			for (int j = 0; j < 3; ++j) {
				sum += v[i][j] * vec.get(j);
			}
			sum += v[i][3];
			out.set(i, sum);
		}
		return out;
	}

  public:
	static Matrix44 identity() {
		Matrix44 M = Matrix44();
		for (size_t i = 0; i < 4; ++i)
			for (size_t j = 0; j < 4; ++j)
				M.v[i][j] = 0;
		for (size_t i = 0; i < 4; ++i)
			M.v[i][i] = 1;
		return M;
	}

	static Matrix44 scale(T s) {
		Matrix44 S = identity();
		for (size_t i = 0; i < 3; ++i)
			S.set(i, i, s);
		return S;
	}

	static Matrix44 translate(T x, T y, T z) {
		Matrix44 M = identity();
		M.set(0, 3, x);
		M.set(1, 3, y);
		M.set(2, 3, z);
		return M;
	}

	static Matrix44 rotateX(T ax) {
		T c = cos(ax);
		T s = sin(ax);
		Matrix44 R = identity();
		R.set(1, 1, c);
		R.set(1, 2, -s);
		R.set(2, 1, s);
		R.set(2, 2, c);
		return R;
	}

	static Matrix44 rotateY(T ay) {
		T c = cos(ay);
		T s = sin(ay);
		Matrix44 R = identity();
		R.set(0, 0, c);
		R.set(0, 2, s);
		R.set(2, 0, -s);
		R.set(2, 2, c);
		return R;
	}

	static Matrix44 rotateZ(T az) {
		T c = cos(az);
		T s = sin(az);
		Matrix44 R = identity();
		R.set(0, 0, c);
		R.set(0, 1, -s);
		R.set(1, 0, s);
		R.set(1, 1, c);
		return R;
	}
};
typedef Matrix44<Number> Matrix44T;

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

template <typename T>
T DegreesToRadians(T a) { return a * M_PI / 180.0; }

template <typename T>
class Transform
{
	Vec3<T> mAng;
    Vec3<T> mPos;

  public:
	Transform() : mAng(), mPos() {}
	Transform(T aPitch, T aYaw, T aRoll, T aX, T aY, T aZ)
	  : mAng(aPitch, aYaw, aRoll), mPos(aX, aY, aZ) {}

	Vec3<T> ang() const { return mAng; }
	Vec3<T> pos() const { return mPos; }

	Matrix44<T> matrix() const {
		return Matrix44T::rotateX(DegreesToRadians(mAng.get(0))) *
				Matrix44T::rotateY(DegreesToRadians(mAng.get(1))) *
				Matrix44T::rotateZ(DegreesToRadians(mAng.get(2))) *
				Matrix44T::translate(mPos.get(0), mPos.get(1), mPos.get(2));
	}
};
typedef Transform<Number> TransformT;

ostream& operator<<(ostream &out, const TransformT &t)
{
	out << "Pitch: " << t.ang().get(0) << ", Yaw: " << t.ang().get(1) << ", Roll: " << t.ang().get(2)
	    << " @ " << t.pos();
	return out;
}

class EventReceiver
{
};

int main(int argc, char **argv)
{
	EventReceiver sink;
	Kinect kinect(sink);

	Vec3T pos(148, 151, 73); // inches
	PointsVector points;
#define PUT(a, b, c, x, y, z) points.push_back(PointMatch(Vec3T(a,b,c), Vec3T(x,y,z)))
	PUT(1847.22, 834.449, 3497.81,   4, 133, 63);
	PUT(-254.13, -275.206, 1307.69,  124, 114, 52);
	PUT(-731.715, -46.0037, 1681.1,  124, 85, 52);
	PUT(123.688, 486.07, 2419.5,     73, 98, 63);
#undef PUT

	// Remap to inches.
	for (PointsIter iter = points.begin(); iter != points.end(); ++iter) {
		/*
		Kinect's axes are reported in mm relative to:
		-> x+
		/\ y+
		out z+
		*/
		Vec3T v(iter->first.get(0), /* e/w */
				iter->first.get(2), /* n/s */
				iter->first.get(1)); /* up/down */
		Vec3T inches = Matrix44T::scale(1/25.4) * iter->first;
		Vec3T rebased = inches - pos;
		iter->first = rebased;
	}

	typedef pair<double, TransformT> Error;
	typedef vector<Error>::iterator ErrorIter;
	vector<Error> minError;
	for (size_t i = 0; i < points.size(); ++i)
		minError.push_back(Error(9999999, TransformT()));

	Timer t;
	LinSpaceT rPitches(0, 359, 360);
	LinSpaceT rYaws(0, 359, 360);
	LinSpaceT rRolls(-0, 0, 1);
	LinSpaceT rX(-0, 0, 1);
	LinSpaceT rY(-0, 0, 1);
	LinSpaceT rZ(-0, 0, 1);
	for (rPitches.begin(); !rPitches.done(); rPitches.next())
		for (rYaws.begin(); !rYaws.done(); rYaws.next())
			for (rRolls.begin(); !rRolls.done(); rRolls.next())
				for (rX.begin(); !rX.done(); rX.next())
					for (rY.begin(); !rY.done(); rY.next())
						for (rZ.begin(); !rZ.done(); rZ.next()) {
							TransformT tr(rPitches.v(), rYaws.v(), rRolls.v(),
											rX.v(), rY.v(), rZ.v());
							Matrix44T M = tr.matrix();

							size_t i = 0;
							for (PointsIter iter = points.begin(); iter != points.end(); ++iter, ++i) {
								Vec3T transformed = M * iter->first;
								Vec3T toTgt = iter->second - transformed;
								double error = toTgt.length();
								if (error < minError[i].first) {
									minError[i].first = error;
									minError[i].second = tr;
								}
							}
						}
	double numprobs = rPitches.count() * rYaws.count() * rRolls.count() * rX.count() * rY.count() * rZ.count();
	for (ErrorIter iter = minError.begin(); iter != minError.end(); ++iter)
		cout << "Minimum is: " << iter->first << " @ " << iter->second << endl;
	cout << "T: " << t.stop() << " nProbs: " << numprobs << endl;
}
