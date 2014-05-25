# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp.typeclass import DerivingEq


def clamp(v, low, high):
    return min(max(low, v), high)


class BHS(DerivingEq):
    """Brightness, hue, saturation; as used by Philips Hue."""
    def __init__(self, b, h, s):
        self.b_ = b  # [0,255]
        self.h_ = h  # [0,64457]
        self.s_ = s  # [0,255]

    @property
    def b(self):
        return clamp(int(self.b_), 0, 255)

    @property
    def h(self):
        return clamp(int(self.h_), 0, 2**16 - 1)

    @property
    def s(self):
        return clamp(int(self.s_), 0, 255)

    def __str__(self):
        return "B:{0.b}, H:{0.h}, S:{0.s}".format(self)


class RGB(DerivingEq):
    """Red, green, blue triple."""
    def __init__(self, r, g, b):
        self.r_ = r  # [0,255]
        self.g_ = g  # [0,255]
        self.b_ = b  # [0,255]

    @property
    def r(self):
        return clamp(int(self.r_), 0, 255)

    @property
    def g(self):
        return clamp(int(self.g_), 0, 255)

    @property
    def b(self):
        return clamp(int(self.b_), 0, 255)

    def __str__(self):
        return "R:{0.r}, G:{0.g}, B:{0.b} | #{0.r:02X}{0.g:02X}{0.b:02X}".format(self)


class Mired(DerivingEq):
    """Mired style color temperature."""
    def __init__(self, ct):
        self.ct_ = ct

    @property
    def ct(self):
        return clamp(self.ct_, 153, 500)

    def __str__(self):
        return "Mired:{}".format(self.ct)