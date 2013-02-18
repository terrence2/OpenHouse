#!/usr/bin/python3
import re
import select
import sys
import zmq

from sensors import *
from servos import *
from floorplan import *
from ruleset import RuleSet
from network import Network

METERS_PER_FOOT = 0.305 # m
METERS_PER_INCH = METERS_PER_FOOT / 12. # m

def m(s):
    """
    Convert an string of form feet'inches" into meters.
    """
    feet = 0
    inches = 0

    s = s.strip()

    feetmatch = re.match(r'^(-?\d+)\'', s)
    if feetmatch:
        feet = float(feetmatch.group(1))
        s = s[len(feetmatch.group(0)):].strip()

    inchesmatch = re.match(r'^(-?\d+)\"', s)
    if inchesmatch:
        inches = float(inchesmatch.group(1))

    return feet * METERS_PER_FOOT + inches * METERS_PER_INCH

class HouseRules(RuleSet):
    def event_BedroomKinect_MAYBEADDUSER(self, sensor, sensorUser):
        # If we were OFF before, then turn on, otherwise, we may already be in
        # a higher-priority state.
        if len(sensor.users) == 1:
            self.floorplan.get_servo('BedLightStrip').color(255, 255, 255)

    def event_BedroomKinect_ADDUSER(self, sensor, sensorUser):
        # This is only here in case we missed the MAYBEADDUSER.
        if len(sensor.users) == 1:
            self.floorplan.get_servo('BedLightStrip').color(255, 255, 255)

    def event_BedroomKinect_REMOVEUSER(self, sensor, sensorUser):
        # Last one out, hit the lights.
        if not sensor.users:
            self.floorplan.get_servo('BedLightStrip').color(0, 0, 0)

    def event_BedroomKinect_POSITION(self, sensor, sensorUser, sensorPos):
        """
        zones = {
            'Desk':
            {'low':  (m(''' 0'93" '''), m(''' 0'59" '''), 0),
             'high': (m(''' 12'4" '''), m(''' 10'7" '''), m(''' 0'57" '''))}
        }
        def is_inside_zone(zone, pos):
            for i in range(3):
                if pos[i] < zone['low'][i] or pos[i] > zone['high'][i]:
                    return False
            return True

        for room in self.floorplan.rooms_with_sensor(sensor):
            roomPos = room.map_sensor_position_to_room_position(sensor, sensorPos)
            inside = [name for name, zone in zones.items() if is_inside_zone(zone, roomPos)]
            if len(inside) == 0:
                self.floorplan.get_servo('BedLightStrip').color(255, 255, 255)
            elif inside[0] == 'Desk':
                self.floorplan.get_servo('BedLightStrip').color(0, 97, 207)

            #print("{}: {} -> {}".format(room.name, roomPos, inside))
        #self.floorplan.get_servo('BedLightStrip').send_test_message()
        """

    def user_ENTERZONE(self, user, zone):
        print("ENTERZONE: {}".format(zone.name))
        zonemap = {
            'Bed': (0, 0, 1),
            'Desk': (0, 97, 207),
        }
        self.floorplan.get_servo('BedLightStrip').color(*zonemap[zone.name])

    def user_LEAVEZONE(self, user, zone):
        print("LEAVEZONE: {}".format(zone.name))
        self.floorplan.get_servo('BedLightStrip').color(255, 255, 255)

def build_floorplan() -> FloorPlan:
    """
      0,0         24'
        +----------+-----------+
        |  11'3"   |           |
        -----------+  Bedroom  |
        | P  |  :  |           | 12'7"
        |-------+  |           |
        |       |HW|  12'4"   k|------+
        |       +  +-----------+      |
    25' | Kitch :              | Porch|
        |       :              |      |
        |             Living   |11'11"|
        |Dining                |      |
        |        Entry         |------+
        +----------------------+  6'7"
    """

    fp = FloorPlan('TrainedMonkeyStudios')
    rules = HouseRules(fp)

    # Add all logical rooms.
    ceiling = m(''' 8' ''')
    bathroom = fp.add_room('Bathroom', m(''' 11'3" '''),  m(''' 4'9" '''),    ceiling)
    bedroom = fp.add_room( 'Bedroom',  m(''' 12'4" '''),  m(''' 12'7" '''),   ceiling)
    pantry = fp.add_room(  'Pantry',   m(''' 4'7" '''),   m(''' 2'6" '''),    ceiling)
    laundry = fp.add_room( 'Laundry',  m(''' 3'1" '''),   m(''' 2'6" '''),    ceiling)
    hallway = fp.add_room( 'Hallway',  m(''' 3' '''),     m(''' 7'7" '''),    ceiling)
    kitchen = fp.add_room( 'Kitchen',  m(''' 8'4" '''),   m(''' 8' '''),      ceiling)
    entry = fp.add_room(   'Entry',    m(''' 3'8" '''),   m(''' 11'11" '''),  ceiling)
    living = fp.add_room(  'Living',   m(''' 12'4"  '''), m(''' 11'11"  '''), ceiling)
    porch = fp.add_room(   'Porch',    m(''' 6'7" '''),   m(''' 11'11" '''),  ceiling)
    dining = fp.add_room(  'Dining',   m(''' 7'7" '''),   m(''' 8'3" '''),    ceiling)

    # Connect our rooms.
    doors = [
        (('''2'4"''', '''6"'''), 'Bathroom', ('''8'11"''', '''4'6"'''), 'Hallway', ('''0'4"''', '''0'-3"''')),
        (('''6"''', '''2'6"'''), 'Bedroom', ('''3"''', '''9'10"'''), 'Hallway', ('''2'9"''', '''6'10"''')),
    ]
    for size, name1, pos1, name2, pos2 in doors:
        room1 = fp.get_room(name1)
        room2 = fp.get_room(name2)
        size = (m(size[0]), m(size[1]))
        pos1 = (m(pos1[0]), m(pos1[1]))
        pos2 = (m(pos2[0]), m(pos2[1]))
        room1.add_portal_to(room2, size, pos1)
        room2.add_portal_to(room1, size, pos2)

    # Add zones to all rooms.
    bedroomZones = {
        'Bed': (
            (m(''' 57" '''), 0, 0),
            (m(''' 134" '''), m(''' 58" '''), m(''' 8' '''))),
        'Desk': (
            (m(''' 93" '''), m(''' 59" '''), 0),
            (m(''' 12'4" '''), m(''' 10'7" '''), m(''' 0'57" '''))),
    }
    for name, extents in bedroomZones.items():
        bedroom.add_zone(Zone(name, extents[0], extents[1]))

    # Add all the sensors.
    sensors = [
        (Kinect, 'BedroomKinect', 'gorilla', Network.DefaultSensorPort,
                [('Bedroom', m(''' 12'4" '''), m(''' 12'7" '''), m(''' 6'1" '''),
                     [-0.0005185897176859047, -0.0003758472848349213, -0.0007688408741131634, 3.66,
                      0.0008539254011549632, -0.0002865791659343441, -0.0004358859454365706, 3.73625,
                      5.647017342997122e-05, 0.0008819999810935458, -0.0004692546423619012, 1.855416666666667,
                      0.0, 0.0, 0.0, 1.0]
                 )])
    ]
    for cls, name, host, port, rooms in sensors:
        s = cls(fp, rules, name, (host, port))
        for roomName, X, Y, Z, registration in rooms:
            fp.add_sensor(s, roomName, (X, Y, Z), registration)

    # Add all servos.
    servos = [
        (LightStrip, 'BedLightStrip', '127.0.0.1', Network.DefaultServoPort, 'Bedroom')
    ]
    for cls, name, host, port, room in servos:
        s = cls(name, (host, port))
        fp.add_servo(s, room)
    #bedLightStrip = LightStrip('BedLightStrip', '127.0.0.1')
    #fp.add_servo(bedLightStrip, 'Bedroom')

    return fp

def main():
    floorplan = build_floorplan()

    network = Network(floorplan)

    return network.run()


if __name__ == '__main__':
    sys.exit(main())
