from unittest import TestCase
from mcp.abode import Abode, Area
from mcp.dimension import Size, Coord

__author__ = 'terrence'


class TestAbode(TestCase):
    def test_create_room(self):
        abode = Abode('Test')
        room1 = abode.create_room('Room1', Coord(0, 0), Size(10, 10, 8))
        room2 = abode.create_room('Room2', Coord(8, 8), Size(10, 10, 8))
        room3 = abode.create_room('Room3', Coord(16, 16), Size(10, 10, 8))
        self.assertIs(room2, abode.room("Room2"))
        self.assertIs(room2, abode.lookup("/Test/Room2"))
        self.assertEqual(room2.position, Coord(8, 8))
        self.assertEqual(room2.size, Size(10, 10, 8))

    def test_create_area(self):
        abode = Abode('Test')
        room = abode.create_room('Room', Coord(0, 0), Size(10, 10, 8))
        area1 = room.create_subarea('Area1', Coord(0, 0), Size(10, 10, 8))
        area2 = room.create_subarea('Area2', Coord(8, 8), Size(10, 10, 8))
        area3 = room.create_subarea('Area3', Coord(16, 16), Size(10, 10, 8))
        self.assertIs(area2, room.subarea("Area2"))
        self.assertIs(area2, abode.lookup("/Test/Room/Area2"))
        self.assertEqual(area2.position, Coord(8, 8))
        self.assertEqual(area2.size, Size(10, 10, 8))

