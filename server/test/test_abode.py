# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from unittest import TestCase
from mcp.abode import Abode, Area
from mcp.dimension import Size, Coord

__author__ = 'terrence'


class TestAbode(TestCase):
    def build_abode(self):
        abode = Abode('Test')
        room1 = abode.create_room('Room1', Coord(0, 0), Size(10, 10, 8))
        room2 = abode.create_room('Room2', Coord(8, 8), Size(10, 10, 8))
        room3 = abode.create_room('Room3', Coord(16, 16), Size(10, 10, 8))
        for room in [room1, room2, room3]:
            area1 = room.create_subarea('Area1', Coord(0, 0), Size(10, 10, 8))
            area2 = room.create_subarea('Area2', Coord(8, 8), Size(10, 10, 8))
            area3 = room.create_subarea('Area3', Coord(16, 16), Size(10, 10, 8))
        return (abode, room1, room2, room3, area1, area2, area3)

    def test_create_room(self):
        abode, room1, room2, room3, _, _, _ = self.build_abode()
        self.assertIs(room2, abode.room("Room2"))
        self.assertIs(room2, abode.lookup("/Test/Room2"))
        self.assertEqual(room2.position, Coord(8, 8))
        self.assertEqual(room2.size, Size(10, 10, 8))

    def test_create_area(self):
        abode, room1, room2, room3, area1, area2, area3 = self.build_abode()
        self.assertIs(area2, room3.subarea("Area2"))
        self.assertIs(area2, abode.lookup("/Test/Room3/Area2"))
        self.assertEqual(area2.position, Coord(8, 8))
        self.assertEqual(area2.size, Size(10, 10, 8))

    def test_path(self):
        abode, room1, room2, room3, area1, area2, area3 = self.build_abode()
        self.assertEqual('/Test', abode.path())
        self.assertEqual('/Test/Room1', room1.path())
        self.assertEqual('/Test/Room2', room2.path())
        self.assertEqual('/Test/Room3', room3.path())
        self.assertEqual('/Test/Room3/Area1', area1.path())
        self.assertEqual('/Test/Room3/Area2', area2.path())
        self.assertEqual('/Test/Room3/Area3', area3.path())
        for area in (abode, room1, room2, room3, area1, area2, area3):
            self.assertIs(area, abode.lookup(area.path()))

    def test_get_and_set(self):
        props = {'foo': 'bar', 'hello': 'world'}
        areas = self.build_abode()
        for area in areas:
            for key, val in props.items():
                area.set(key, val)
        for area in areas:
            for key, val in props.items():
                self.assertEqual(val, area.get(key))

    def test_set_events(self):
        call_count = 0
        def receiver(expect_event: str, expect_name: str, expect_value: str):
            nonlocal call_count
            def callback(event):
                nonlocal call_count
                self.assertEqual(expect_event, event.event_name)
                self.assertEqual(expect_name, event.property_name)
                self.assertEqual(expect_value, event.property_value)
                call_count += 1
            return callback

        props = {'foo': 'bar', 'hello': 'world'}
        areas = self.build_abode()

        # Check that initial update fires both added and changed.
        for area in areas:
            for key, val in props.items():
                area.listen(key, 'propertyAdded', receiver('propertyAdded', key, val))
                area.listen(key, 'propertyChanged', receiver('propertyChanged', key, val))
        for area in areas:
            for key, val in props.items():
                area.set(key, val)
        self.assertEqual(len(areas) * len(props) * 2, call_count)
        for area in areas:
            for key, val in props.items():
                self.assertEqual(val, area.get(key))
        self.assertEqual(len(areas) * len(props) * 2, call_count)  # No new calls when getting.

        # Check that setting the key to the same value does not fire new added/changed events.
        for area in areas:
            for key, val in props.items():
                area.set(key, val)
        self.assertEqual(len(areas) * len(props) * 2, call_count)  # No new calls when getting.

        # Add touch events and re-set to see if those fire correctly.
        for area in areas:
            for key, val in props.items():
                area.listen(key, 'propertyTouched', receiver('propertyTouched', key, val))
        for area in areas:
            for key, val in props.items():
                area.set(key, val)
        self.assertEqual(len(areas) * len(props) * 3, call_count)



