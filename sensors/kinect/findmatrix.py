#!/usr/bin/python3
from matrix import matrix

import cmd
import math
import os
import os.path
import pickle
import re
import readline
import subprocess
import sys

class Coord:
    def __init__(self, x=0, y=0, z=0, units=''):
        self.x = x
        self.y = y
        self.z = z
        self.units = units

    def __getitem__(self, i):
        assert 0 <= i < 3
        if i == 0: return self.x
        elif i == 1: return self.y
        elif i == 2: return self.z
        raise ValueError(i)

    def __add__(self, other):
        assert self.units == other.units
        return Coord(self.x + other.x, self.y + other.y, self.z + other.z)

    def length(self):
        return math.sqrt(x * x + y * y + z * z)

    def raw(self):
        return '{} {} {}'.format(self.x, self.y, self.z).encode('ASCII')

    def __str__(self):
        return "{0}{3}, {1}{3}, {2}{3}".format(self.x, self.y, self.z, self.units)

class PointMatch:
    def __init__(self, measured, reference):
        self.measured = measured
        self.actual = reference + measured
        self.sensor = Coord()

    def raw(self):
        return self.sensor.raw() + b' ' + self.measured.raw()

    def __str__(self):
        return "({} [{}]) <-> ({})".format(str(self.actual), str(self.measured), str(self.sensor))

class Interp(cmd.Cmd):
    prompt = '> '
    VERSION = 1 # file format version

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.matrix = []
        self.sensor = Coord(0, 0, 0, '"')
        self.reference = Coord(0, 0, 0, '"')
        self.points = []
        if os.path.exists('session.sav'):
            self.do_load('session.sav')

    def parse_coord(self, line, unit, fmt):
        match = re.match(r'(-?\d+)\s+(-?\d+)\s+(-?\d+)', line)
        if not match:
            print("Unrecognized input -- format is '{}'".format(fmt))
            return None
        x, y, z = (int(match.group(i)) for i in range(1,4))
        return Coord(x, y, z, unit)

    def parse_index(self, line, fmt):
        try:
            index = int(line)
        except ValueError:
            print("Unrecognized input -- format is '{}'".format(fmt))
            return None
        if index < 0 or index >= len(self.points):
            print("Index out of range.")
            return None
        return index

    def postcmd(self, stop, line):
        self.do_save('session.sav')
        return line == 'EOF'

    def do_EOF(self, line):
        "Exit the program."
        print()

    def do_show(self, line):
        "Show the currently collected list of points."
        print("Kinect at: " + str(self.sensor))
        print("Measurement Reference Point: " + str(self.reference))
        print("Point list:")
        for i, pt in enumerate(self.points):
            print(i, ': ', str(pt))
        print("Current Matrix:")
        print(self.matrix)

    def do_matrix(self, line):
        child = subprocess.Popen(['./register'], stdin=subprocess.PIPE, stderr=subprocess.PIPE)
        child.stdin.write(self.sensor.raw() + b'\n')
        child.stdin.write(self.reference.raw() + b'\n')
        for i, pt in enumerate(self.points):
            child.stdin.write(pt.raw() + b' ')
        _, stderr = child.communicate()
        nums = stderr.decode('ASCII').split()
        nums = [float(n.strip()) for n in nums]
        self.matrix = nums
        print("Matrix: ", self.matrix)

    def do_add(self, line):
        "Add a point to the system - format X Y Z in inches from the reference point."
        coord = self.parse_coord(line, '"', 'add X Y Z')
        if not coord:
            return
        self.points.append(PointMatch(coord, self.reference))
        print('Added: {}'.format(coord))

    def do_remove(self, line):
        index = self.parse_index(line, 'remove i')
        if index is None:
            return
        self.points = [pt for i, pt in enumerate(self.points) if i != index]
        print('Removed {}'.format(index))

    def do_capture(self, line):
        "Run the sensor and update the sensor position of an existing point."
        index = self.parse_index(line, 'capture i')
        if index is None:
            return
        child = subprocess.Popen(['./picker'], stderr=subprocess.PIPE, bufsize=4096)
        _, stderr = child.communicate()
        if child.returncode != 0:
            print("Picker exited with non-zero return code, probably a crash.")
            print("Try re-connecting the kinect.")
            return
        match = re.match(r'\((-?\d+\.\d+),\s+(-?\d+\.\d+),\s+(-?\d+\.\d+)\)', stderr.decode('UTF-8'))
        if not match:
            print("Unable to parse output: raw is {}".format(stderr))
            return
        x, y, z = (float(match.group(i)) for i in range(1,4))
        self.points[index].sensor = Coord(x, y, z, 'mm')
        print("Index {} updated: {}".format(index, str(self.points[index])))

    def do_sensor(self, line):
        "Set the position of the sensor."
        coord = self.parse_coord(line, '"', 'sensor X Y Z')
        if not coord:
            return
        self.sensor = coord
        print("New sensor position: " + str(self.sensor))

    def do_inches(self, line):
        matches = re.match(r'(\d+)\'(\d+)"', line)
        if not matches:
            print("Unrecognized input.")
        total = 12 * int(matches.group(1)) + int(matches.group(2))
        print("As inches: {}\"".format(total))

    def do_reference(self, line):
        "Set a reference coordinate."
        coord = self.parse_coord(line, '"', 'reference X Y Z')
        if not coord:
            return
        self.reference = coord
        print("New reference position: " + str(self.reference))

    def do_load(self, line):
        "Load state from file."
        fn = os.path.realpath(line)
        with open(fn, 'rb') as fp:
            data = pickle.load(fp)
        if data[0] != self.VERSION:
            print("Loaded save is from wrong version {}, current is {}".format(v, self.VERSION))
        self.sensor, self.reference, self.points, self.matrix = data[1:]
        print("Loaded: {}".format(fn))

    def do_save(self, line):
        "Save state to file."
        fn = os.path.realpath(line)
        with open(fn, 'wb') as fp:
            pickle.dump((self.VERSION, self.sensor, self.reference, self.points, self.matrix), fp)

if __name__ == '__main__':
    Interp().cmdloop("Iterate to find the right matrix:")
