# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import math

Raw = int or float or str
FeetToMeters = 0.305
InchesToMeters = FeetToMeters / 12.0


def from_human_dimension(raw: Raw, acc=0.0):
    def unit_size(s):
        return {'m': 1.0,
                'ft': FeetToMeters,
                'in': InchesToMeters,
                }[s]

    def is_num(c):
        assert isinstance(c, str)
        assert len(c) == 1
        return c in "01234567890."

    def parse_num(s):
        num_str = ""
        while s and is_num(s[0]):
            num_str += s[0]
            s = s[1:]
        return float(num_str), s

    def parse_unit(s):
        unit_str = ""
        while s and not is_num(s[0]):
            unit_str += s[0]
            s = s[1:]
        return unit_size(unit_str), s

    if isinstance(raw, (int, float)):
        return raw
    if not raw:
        return acc
    sz, raw = parse_num(raw)
    ratio, raw = parse_unit(raw)
    return from_human_dimension(raw, acc + sz * ratio)


def humanize_dimension(dim: int or float):
    feet = dim / FeetToMeters
    inches = (feet - math.floor(feet)) / InchesToMeters
    feet = math.floor(feet)
    inches = math.floor(inches)
    if feet < 1:
        return "{}in".format(inches)
    if inches < 1:
        return "{}ft".format(feet)
    return "{}ft{}in".format(feet, inches)



class Size:
    def __init__(self, x: Raw, y: Raw, z: Raw):
        self.x = from_human_dimension(x)
        self.y = from_human_dimension(y)
        self.z = from_human_dimension(z)

    def __str__(self):
        return "{} x {} x {}".format(humanize_dimension(self.x),
                                     humanize_dimension(self.y),
                                     humanize_dimension(self.z))

    def __eq__(self, other):
        return self.x == other.x and self.y == other.y and self.z == other.z

class Coord(Size):
    def __init__(self, x: Raw, y: Raw, z: Raw = 0):
        super().__init__(x, y, z)

    def __str__(self):
        if z == 0:
            return "{} x {}".format(humanize_dimension(self.x),
                                    humanize_dimension(self.y))
        return super().__str__()

