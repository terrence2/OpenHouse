#!/usr/bin/python3
import matrix
import math
import sys
import os

def in_to_mm(inches):
	return inches * 25.4 # Note: not an approximation, this is the actual definition of an inch now

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

def get_mse(ax, ay, az, pos, kp, wp):
	"""
	Kinect's axes are reported in mm relative to:
	-> x+
	/\ y+
	out z+

	Given a position and anti-clockwise rotations Rx, Ry, Rz (in degrees, not
	radians), compute how close each transformed kp[i] is to its known
	real-world position wp[i].
	"""
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

	kpt = matrix.vector(kp[0] + [1])
	kpt = kpt.transpose()
	print(kpt)
	return Rx * Ry * Rz * T * kpt


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
