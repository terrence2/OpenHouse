# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import colorsys
import logging
from pprint import pprint
import re

from shared.typeclass import DerivingEq, DerivingAdd, DerivingMul, DerivingSub


log = logging.getLogger('color')


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


def parse_css_color(style: str) -> Color:
    style = style.strip().lower()
    if style.startswith('#'):
        if len(style) == 4:
            """Strings of the form #RGB"""
            return RGB(
                int(style[1] + style[1], 16),
                int(style[2] + style[2], 16),
                int(style[3] + style[3], 16)
            )
        elif len(style) == 7:
            """Strings of the form #RRGGBB"""
            return RGB(
                int(style[1:3], 16),
                int(style[3:5], 16),
                int(style[5:7], 16)
            )
        log.error("Mis-formatted color string, expected #RGB or #RRGGBB.")
        raise Exception("Invalid color string, expected #RGB or #RRGGBB.")
    else:
        matches = re.match(r'([a-z]+)\s*\(([0-9,\s]+)\)', style)
        if not matches:
            log.error("Mis-formatted color string.")
            raise Exception("Invalid color string")
        if matches.group(1) == 'rgb':
            """Strings of the form rgb(r,g,b)"""
            parts = matches.group(2).split(',')
            return RGB(int(parts[0].strip()),
                       int(parts[1].strip()),
                       int(parts[2].strip()))
        elif matches.group(1) == 'bhs':
            """Strings of the form bhs(b,h,s)"""
            parts = matches.group(2).strip().split(',')
            return BHS(int(parts[0].strip()),
                       int(parts[1].strip()),
                       int(parts[2].strip()))
        elif matches.group(1) == 'mired':
            """Strings of the form mired(ct)"""
            ct = int(matches.group(2).strip())
            return Mired(ct)
        log.error("Mis-formatted color string, expected type(component...).")
        raise Exception("Invalid color string, expected type(component...)")

