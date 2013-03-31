import logging

from lib import *
from sensors import *
from servos import *
from floorplan import *
from ruleset import RuleSet

log = logging.getLogger('TMS')

class HouseRules(RuleSet):
    def event_BedroomKinect_MAYBEADDUSER(self, sensor, sensorUser, uid):
        log.info("MAYBEADDUSER {} -> {}".format(uid, sensor.users))
        # Speculatively flip on the lights as soon as we get a track.
        if not self.floorplan.room_has_users(self.floorplan.get_room('Bedroom')):
            self.floorplan.get_servo('BedLightStrip').color(255, 255, 255)

    def event_BedroomKinect_ADDUSER(self, sensor, sensorUser): pass
    def event_BedroomKinect_REMOVEUSER(self, sensor, sensorUser): pass
    def event_BedroomKinect_POSITION(self, sensor, sensorUser, sensorPos): pass

    def user_ENTERZONE(self, user, zone):
        log.info("ENTERZONE: {}".format(zone.name))
        zonemap = {
            'Bed': (0, 0, 1),
            'Desk': (0, 97, 207),
        }
        self.floorplan.get_servo('BedLightStrip').color(*zonemap[zone.name])

    def user_LEAVEZONE(self, user, zone):
        log.info("LEAVEZONE: {}".format(zone.name))
        self.floorplan.get_servo('BedLightStrip').color(255, 255, 255)

    def user_CHANGEROOM(self, user, priorRoom, currentRoom):
        n1 = priorRoom.name if priorRoom else 'None'
        n2 = currentRoom.name if currentRoom else 'None'
        log.info("CHANGEROOM: {} -> {}".format(n1, n2))
        if not currentRoom: # and len(self.floorplan.users_in_room(currentRoom)) == 1:
            self.floorplan.get_servo('BedLightStrip').color(50, 0, 0)
        else:
            self.floorplan.get_servo('BedLightStrip').color(255, 255, 255)

    def user_REMOVEUSER(self, user):
        # FIXME: implement level triggers.
        bedroom = self.floorplan.get_room('Bedroom')
        if not self.floorplan.room_has_users(bedroom):
            self.floorplan.get_servo('BedLightStrip').color(50, 0, 0)


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
            (m(''' 42" '''), 0, 0),
            (m(''' 124" '''), m(''' 68" '''), m(''' 8' '''))),
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
