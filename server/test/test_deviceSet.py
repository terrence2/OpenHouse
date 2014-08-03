# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from unittest import TestCase

__author__ = 'terrence'

from mcp.devices import DeviceSet
from mcp import Device


class MyDev(Device):
    def __init__(self, name):
        super().__init__(name)
        self.propname = 'initial'


class TestDeviceSet(TestCase):
    def test_select(self):
        devices = DeviceSet()

        f00 = devices.add(MyDev("foo-room0-name0"))
        f01 = devices.add(MyDev("foo-room0-name1"))
        f02 = devices.add(MyDev("foo-room0-name2"))
        r00 = devices.add(MyDev("bar-room0-name0"))
        r01 = devices.add(MyDev("bar-room0-name1"))
        r02 = devices.add(MyDev("bar-room0-name2"))
        z00 = devices.add(MyDev("baz-room0-name0"))
        z01 = devices.add(MyDev("baz-room0-name1"))
        z02 = devices.add(MyDev("baz-room0-name2"))

        f10 = devices.add(MyDev("foo-room1-name0"))
        f11 = devices.add(MyDev("foo-room1-name1"))
        f12 = devices.add(MyDev("foo-room1-name2"))
        r10 = devices.add(MyDev("bar-room1-name0"))
        r11 = devices.add(MyDev("bar-room1-name1"))
        r12 = devices.add(MyDev("bar-room1-name2"))
        z10 = devices.add(MyDev("baz-room1-name0"))
        z11 = devices.add(MyDev("baz-room1-name1"))
        z12 = devices.add(MyDev("baz-room1-name2"))

        f20 = devices.add(MyDev("foo-room2-name0"))
        f21 = devices.add(MyDev("foo-room2-name1"))
        f22 = devices.add(MyDev("foo-room2-name2"))
        r20 = devices.add(MyDev("bar-room2-name0"))
        r21 = devices.add(MyDev("bar-room2-name1"))
        r22 = devices.add(MyDev("bar-room2-name2"))
        z20 = devices.add(MyDev("baz-room2-name0"))
        z21 = devices.add(MyDev("baz-room2-name1"))
        z22 = devices.add(MyDev("baz-room2-name2"))

        self.assertEqual(f00.name, "foo-room0-name0")

        devices.select("*").set(propname='value0')
        for dev in devices:
            self.assertEqual(dev.propname, "value0")

        devices.select("@room0").set(propname='value1')
        for dev in (f00, f01, f02, r00, r01, r02, z00, z01, z02):
            self.assertEqual(dev.propname, "value1")

        devices.select("$foo").set(propname='value2')
        for dev in (f00, f01, f02, f10, f11, f12, f20, f21, f22):
            self.assertEqual(dev.propname, "value2")

        devices.select("#name0").set(propname='value3')
        for dev in (f00, r00, z00, f10, r10, z10, f20, r20, z20):
            self.assertEqual(dev.propname, "value3")

        d = devices.select("@room1").select("$bar").select("#name2")
        self.assertEqual(len(d), 1)
        self.assertTrue(d)
        d.set(propname='value4')
        self.assertEqual(r12.propname, 'value4')

        d = devices.select("@room99999")
        self.assertEqual(len(d), 0)
        self.assertFalse(d)

        d = devices.select("@room0")
        d.set(propname='before')
        (d - d.select("$baz")).set(propname='value5')
        for dev in (z00, z01, z02):
            self.assertEqual(dev.propname, 'before')
        for dev in (f00, f01, f02, r00, r01, r02):
            self.assertEqual(dev.propname, 'value5')

        self.assertFalse(devices.select(""))
        self.assertFalse(devices.select(None))

