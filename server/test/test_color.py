# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from unittest import TestCase

from mcp.color import BHS


class TestBHS(TestCase):
    def test_create(self):
        c = BHS(0, 0, 0)
        self.assertEqual(c.b, 0)
        self.assertEqual(c.h, 0)
        self.assertEqual(c.s, 0)

        c = BHS(255, 2**16-1, 255)
        self.assertEqual(c.b, 255)
        self.assertEqual(c.h, 2**16-1)
        self.assertEqual(c.s, 255)

    def test_clamp(self):
        c = BHS(-1, -1, -1)
        self.assertEqual(c.b, 0)
        self.assertEqual(c.h, 2**16-1)
        self.assertEqual(c.s, 0)

        c = BHS(256, 2**16, 256)
        self.assertEqual(c.b, 255)
        self.assertEqual(c.h, 0)
        self.assertEqual(c.s, 255)

    def test_sub(self):
        a = BHS(0, 0, 0)
        b = BHS(255, 2**16-1, 255)
        c = b - a
        self.assertEqual(c.b, 255)
        self.assertEqual(c.h, 2**16-1)
        self.assertEqual(c.s, 255)
        c = a - b
        self.assertEqual(c.b, 0)
        self.assertEqual(c.h, 1)
        self.assertEqual(c.s, 0)

    def test_add(self):
        a = BHS(0, 0, 0)
        b = BHS(255, 2**16-1, 255)
        c = b + a
        self.assertEqual(c.b, 255)
        self.assertEqual(c.h, 2**16-1)
        self.assertEqual(c.s, 255)
        c = a + b
        self.assertEqual(c.b, 255)
        self.assertEqual(c.h, 2**16-1)
        self.assertEqual(c.s, 255)
        c = b + b + b
        self.assertEqual(c.b, 255)
        self.assertEqual(c.h, 2**16-3)
        self.assertEqual(c.s, 255)

    def test_mul(self):
        a = BHS(0, 0, 0)
        b = BHS(255, 2**16-1, 255)
        c = a * 100000
        self.assertEqual(c.b, 0)
        self.assertEqual(c.h, 0)
        self.assertEqual(c.s, 0)
        c = b * 0
        self.assertEqual(c.b, 0)
        self.assertEqual(c.h, 0)
        self.assertEqual(c.s, 0)
        d = BHS(5, 5, 5)
        c = d * 5
        self.assertEqual(c.b, 5)
        self.assertEqual(c.h, 5)
        self.assertEqual(c.s, 5)


