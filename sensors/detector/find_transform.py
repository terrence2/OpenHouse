#!/usr/bin/python3
import math
import sys
import os
import numpy as np

def in_to_mm(inches):
	return inches * 25.4 # Note: not an approximation, this is the actual definition of an inch now

def mm_to_in(mm):
	return mm / 25.4

def deg_to_rad(deg):
	return deg * math.pi / 180.0

def read_map(mapfile):
	# Read the map
	with open(mapfile) as fp:
		lines = fp.readlines()

	# Remove comments
	tmp = []
	for line in lines:
		if line.find('#') >= 0:
			line = line[:line.find('#')]
		line = line.strip()
		if line:
			tmp.append(line)
	lines = tmp

	# First line is position:
	assert lines[0].startswith('position:')
	pos = [float(p.strip().rstrip('"')) for p in lines[0][len('position:'):].split(',')]
	assert len(pos) == 3
	lines = lines[1:]

	# Process and split coordinates.
	kp = []
	wp = []
	for line in lines:
		parts = [p.strip() for p in line.split(',')]
		assert len(parts) > 6
		for p in parts[3:6]: assert p.endswith('"')

		kpt = [float(p) for p in parts[:3]]
		wpt = [float(p.rstrip('"')) for p in parts[3:6]]
		kp.append(kpt)
		wp.append(wpt)
	
	return pos, kp, wp

def dist(a, b):
	x = a[0][0] - b[0][0]
	y = a[1][0] - b[1][0]
	z = a[2][0] - b[2][0]
	return math.sqrt(x**2 + y**2 + z**2)


def identity():
	I = np.zeros((4, 4), float)
	for i in range(4):
		I[i][i] = 1.0
	return I

def scale(s):
	S = np.zeros((4, 4), float)
	for i in range(3):
		S[i][i] = s
	S[3][3] = 1.0
	return np.asmatrix(S)

def transform(pos):
	T = identity()
	T[0][3] = pos[0]
	T[1][3] = pos[1]
	T[2][3] = pos[2]
	return np.asmatrix(T)

def rotateX(ax):
	Rx = identity()
	Rx[1][1] = math.cos(ax)
	Rx[1][2] = -math.sin(ax)
	Rx[2][1] = math.sin(ax)
	Rx[2][2] = math.cos(ax)
	return np.asmatrix(Rx)

def rotateY(ay):
	Ry = identity()
	Ry[0][0] = math.cos(ay)
	Ry[0][2] = math.sin(ay)
	Ry[2][0] = -math.sin(ay)
	Ry[2][2] = math.cos(ay)
	return np.asmatrix(Ry)

def rotateZ(az):
	Rz = identity()
	Rz[0][0] = math.cos(az)
	Rz[0][1] = -math.sin(az)
	Rz[1][0] = math.sin(az)
	Rz[1][1] = math.cos(az)
	return np.asmatrix(Rz)

def vector(inp):
	v = np.zeros((4), float)
	v[0] = inp[0]
	v[1] = inp[1]
	v[2] = inp[2]
	v[3] = 1.0
	return np.asmatrix(v)

def get_mse(ax, ay, az, pos, kp, wp):
	"""
	Kinect's axes are reported in mm relative to:
	-> x+
	/\ y+
	out z+

	Given a position and anti-clockwise rotations Rx, Ry, Rz (in degrees, not
	radians), compute how close each transformed kp[i] is to its known
	real-world position wp[i].
	Rx = matrix.identity(4)
	Rx[1,1] = math.cos(ax)
	Rx[2,1] = -math.sin(ax)
	Rx[1,2] = math.sin(ax)
	Rx[2,2] = math.cos(ax)

	Ry = matrix.identity(4)
	Ry[0,0] = math.cos(ay)
	Ry[2,0] = math.sin(ay)
	Ry[0,2] = -math.sin(ay)
	Ry[2,2] = math.cos(ay)

	Rz = matrix.identity(4)
	Rz[0,0] = math.cos(az)
	Rz[1,0] = -math.sin(az)
	Rz[0,1] = math.sin(az)
	Rz[1,1] = math.cos(az)

	T = matrix.identity(4)
	T[3,0] = pos[0]
	T[3,1] = pos[1]
	T[3,2] = pos[2]
	"""

	"""
	for i in range(len(kp)):
		kpt = matrix.vector(kp[i] + [1]).transpose()
		wpt = matrix.vector(wp[i] + [1]).transpose()
		m = (9**88, -1, -1)
		for pitch in range(-40, -20):
			for yaw in range(360):
				tpt = (
						matrix.rotateX(deg_to_rad(pitch)) * 
						matrix.rotateY(deg_to_rad(yaw)) * 
						matrix.transform((-pos[0], pos[2], -pos[1])) *
						matrix.scale(1/25.4) * 
						kpt)
				d = dist(tpt, wpt)
				print("pitch {}, yaw {}: dist: {}".format(pitch, yaw, d))
				if d < m[0]:
					m = (d, pitch, yaw)
		print("{}: min: {}\" at pitch: {}, yaw: {}".format(i, *m))
	"""
	for i in range(len(kp)):
		kpt = vector(kp[i]).transpose()
		wpt = vector(wp[i]).transpose()
		m = (9**88, -1, -1)
		for pitch in range(-40, -20):
			for yaw in range(360):
				tpt = (
						rotateX(deg_to_rad(pitch)) * 
						rotateY(deg_to_rad(yaw)) * 
						transform((-pos[0], pos[2], -pos[1])) *
						scale(1/25.4) * 
						kpt)
				d = dist(tpt, wpt)
				#print("pitch {}, yaw {}: dist: {}".format(pitch, yaw, d))
				if d < m[0]:
					m = (d, pitch, yaw)
		print("{}: min: {}\" at pitch: {}, yaw: {}".format(i, *m))





def main():

	# Load coordinate map.
	pos, kp, wp = read_map(sys.argv[1])

	print(pos)
	for kpt, wpt in zip(kp, wp):
		print("{} -> {}".format(kpt, wpt))

	print(get_mse(340, 0, 210, pos, kp, wp))
	return 0

	for Rx in range(360):
		for Ry in range(360):
			for Rz in range(360):
				e = get_mse(Rx, Ry, Rz, pos, kp, wp)
				print(e)


if __name__ == '__main__':
	sys.exit(main())
