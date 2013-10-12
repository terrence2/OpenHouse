import numpy
import re

METERS_PER_FOOT = 0.305  # m
METERS_PER_INCH = METERS_PER_FOOT / 12.  # m


def m(s):
    """
    Convert an string of form feet'inches" into meters.
    """
    feet = 0
    inches = 0

    s = s.strip()

    feetmatch = re.match(r'^(-?\d+)\'', s)
    if feetmatch:
        feet = float(feetmatch.group(1))
        s = s[len(feetmatch.group(0)):].strip()

    inchesmatch = re.match(r'^(-?\d+)\"', s)
    if inchesmatch:
        inches = float(inchesmatch.group(1))

    return feet * METERS_PER_FOOT + inches * METERS_PER_INCH


def registration_to_matrix(nums):
    A = numpy.array(nums, dtype=float, order='C')
    return A.reshape((4, 4))


def vec4(x, y, z, w=1):
    return numpy.array((x, y, z, w), dtype=float, order='C')
