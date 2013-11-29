import numpy
import re

METERS_PER_FOOT = 0.305  # m
METERS_PER_INCH = METERS_PER_FOOT / 12.  # m

class Dim:
    """
    A dimensioned number. This currently supports feet (ft),
    inches (in), and meters (m) for numbers in the form of
    NuN'u'N"u", for N a number and u a unit descriptor.
    Each N in the list is additive.

    Example: 6ft9in == 72in
    """
    @classmethod
    def parse(cls, s, acc=0):
        def unit_size(s):
            return {'m': 1.0,
                    'ft': 0.305,
                    'in': 0.305 / 12.0,
                    }[s]

        def is_num(c):
            assert isinstance(c, str)
            assert len(c) == 1
            return c in "01234567890."

        def parse_num(s):
            numStr = ""
            while s and is_num(s[0]):
                numStr += s[0]
                s = s[1:]
            return float(numStr), s

        def parse_unit(s):
            unitStr = ""
            while s and not is_num(s[0]):
                unitStr += s[0]
                s = s[1:]
            return unit_size(unitStr), s

        if isinstance(s, (int, float)):
            return s
        if not s:
            return acc
        sz, s = parse_num(s)
        ratio, s = parse_unit(s)
        acc += sz * ratio
        return cls.parse(s, acc + sz * ratio)

    def __init__(self, s: str or Dim):
        if isinstance(s, str):
            self.value_ = self.parse(s)
        else:
            assert isinstance(s, Dim)
            self.value_ = s

    def __str__(self):
        return "{}m".format(self.value_)


class Dim3:
    """
    Rectilinear dimentions, as useful for a floorplan.
    """
    def __init__(self, width: str, length: str, height: str):
        self.width_ = Dim(width)
        self.length_ = Dim(length)
        self.height_ = Dim(height)

    def __str__(self):
        return "{}W{}L{}H".format(str(self.width_),
                                  str(self.length_),
                                  str(self.height_))


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

