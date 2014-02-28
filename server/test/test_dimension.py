from unittest import TestCase
from mcp.dimension import Size, Coord

__author__ = 'terrence'


class TestDimension(TestCase):
    def test_size(self):
        s = Size(0, 1, 2)
        self.assertEqual(s.x, 0)
        self.assertEqual(s.y, 1)
        self.assertEqual(s.z, 2)

        s = Size('0m', '1m', '2m')
        self.assertEqual(s.x, 0)
        self.assertEqual(s.y, 1)
        self.assertEqual(s.z, 2)

        s = Size('0ft', '1ft', '2ft')
        self.assertEqual(s.x, 0 * 0.305)
        self.assertEqual(s.y, 1 * 0.305)
        self.assertEqual(s.z, 2 * 0.305)

        s = Size('0in', '1in', '2in')
        self.assertEqual(s.x, 0 * 0.305 / 12)
        self.assertEqual(s.y, 1 * 0.305 / 12)
        self.assertEqual(s.z, 2 * 0.305 / 12)

    def test_coord(self):
        c = Coord(0, 1)
        self.assertEqual(c.x, 0)
        self.assertEqual(c.y, 1)

        c = Coord('0m', '1m')
        self.assertEqual(c.x, 0)
        self.assertEqual(c.y, 1)

        c = Coord('0ft', '1ft')
        self.assertEqual(c.x, 0 * 0.305)
        self.assertEqual(c.y, 1 * 0.305)

        c = Coord('0in', '1in')
        self.assertEqual(c.x, 0 * 0.305 / 12)
        self.assertEqual(c.y, 1 * 0.305 / 12)

    def test_equality(self):
        self.assertEqual(Coord(8, 8), Coord(8, 8, 0))
        self.assertEqual(Size(2, 4, 8), Size('2m', '4m', '8m'))

