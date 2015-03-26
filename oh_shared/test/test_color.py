from unittest import TestCase
from shared.color import parse_css_color, Color, RGB, BHS, Mired


class TestColor(TestCase):
    def test_parse_css_color(self):
        rgb = parse_css_color("#F0F")
        self.assertEqual(rgb, RGB(255, 0, 255))

        rgb = parse_css_color("#FF00FF")
        self.assertEqual(rgb, RGB(255, 0, 255))

        rgb = parse_css_color("   rgb   (   255,    0,    255   )   ")
        self.assertEqual(rgb, RGB(255, 0, 255))

        bhs = parse_css_color("   bhs  (   255,    32767,    255   )   ")
        self.assertEqual(bhs, BHS(255, 32767, 255))

        mired = parse_css_color("   mired (   500   )   ")
        self.assertEqual(mired, Mired(500))
