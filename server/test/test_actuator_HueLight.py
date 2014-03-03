import math
import time
from unittest import TestCase
from mcp.actuators.hue import HueLight, HueBridge

__author__ = 'terrence'


class TestHueLight(TestCase):
    def setUp(self):
        self.bridge = HueBridge('hue-bedroom', 'MasterControlProgram')
        self.light = HueLight('hue-bedroom-bed', self.bridge, 1)

    def tearDown(self):
        pass

    def test_on(self):
        self.light.on = False
        self.assertEqual(False, self.light.on)
        time.sleep(1)
        self.light.on = True
        self.assertEqual(True, self.light.on)
        time.sleep(1)

    def test_colortemp(self):
        slices = 10
        for v in range(153, 500, math.ceil((500 - 153) / slices)):
            self.light.colortemp = v
            self.assertEqual(v, self.light.colortemp)
            time.sleep(0.01)

        for v in range(500, 153, -math.ceil((500 - 153) / slices)):
            self.light.colortemp = v
            self.assertEqual(v, self.light.colortemp)
            time.sleep(0.01)

    """
    def test_hsv(self):
        self.fail()

    def test_rgb(self):
        self.fail()
    """

