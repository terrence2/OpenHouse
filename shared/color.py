# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import colorsys

from typeclass import DerivingEq, DerivingAdd, DerivingMul, DerivingSub

import tinycss.color3


def _clamp(v, low, high):
    return min(max(v, low), high)


def _wrap(v, high):
    return v % high


class Color:
    pass


class BHS(Color, DerivingEq, DerivingAdd, DerivingMul, DerivingSub):
    """Brightness, hue, saturation; as used by Philips Hue."""
    def __init__(self, b_, h_, s_):
        super().__init__()
        self.b_ = b_  # [0,255]
        self.h_ = h_  # [0,64457]
        self.s_ = s_  # [0,255]

    @property
    def b(self):
        return _clamp(int(self.b_), 0, 255)

    @property
    def h(self):
        return _wrap(int(self.h_), 2**16)

    @property
    def s(self):
        return _clamp(int(self.s_), 0, 255)

    def __str__(self):
        return "BHS({0.b}, {0.h}, {0.s})".format(self)

    @classmethod
    def from_rgb(cls, rgb):
        r = rgb.r / 256
        g = rgb.g / 256
        b = rgb.b / 256
        hue, light, sat = colorsys.rgb_to_hls(r, g, b)
        return cls(light * 256, hue * 2**16, sat * 256)

    @classmethod
    def from_css(cls, rgba: tinycss.color3.RGBA):
        hue, light, sat = colorsys.rgb_to_hls(rgba.red, rgba.green, rgba.blue)
        return cls(light * 256, hue * 2**16, sat * 256)


class RGB(Color, DerivingEq, DerivingAdd, DerivingMul, DerivingSub):
    """Red, green, blue triple."""
    def __init__(self, r_, g_, b_):
        super().__init__()
        self.r_ = r_  # [0,255]
        self.g_ = g_  # [0,255]
        self.b_ = b_  # [0,255]

    @property
    def r(self):
        return _clamp(int(self.r_), 0, 255)

    @property
    def g(self):
        return _clamp(int(self.g_), 0, 255)

    @property
    def b(self):
        return _clamp(int(self.b_), 0, 255)

    def __str__(self):
        return "RGB({0.r}, {0.g}, {0.b}) or #{0.r:02X}{0.g:02X}{0.b:02X}".format(self)

    @classmethod
    def from_bhs(cls, bhs: BHS):
        bri = bhs.b / 256
        hue = bhs.h / (2**16)
        sat = bhs.s / 256
        r, g, b = colorsys.hls_to_rgb(hue, bri, sat)
        return cls(r * 256, g * 256, b * 256)


class Mired(Color, DerivingEq, DerivingAdd, DerivingMul, DerivingSub):
    """Mired style color temperature."""
    def __init__(self, ct_):
        super().__init__()
        self.ct_ = ct_

    @property
    def ct(self):
        return _clamp(self.ct_, 153, 500)

    def __str__(self):
        return "Mired({})".format(self.ct)