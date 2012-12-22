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

class HouseRules(RuleSet):
	def event_BedroomKinect_MAYBEADDUSER(self, sensor):
		# If we were OFF before, then turn on, otherwise, we may already be in
		# a higher-priority state.
		if len(sensor.users) == 1:
			self.floorplan.get_servo('BedLightStrip').turn_on_full()

	def event_BedroomKinect_ADDUSER(self, sensor):
		# This is only here in case we missed the MAYBEADDUSER.
		if len(sensor.users) == 1:
			self.floorplan.get_servo('BedLightStrip').turn_on_full()

	def event_BedroomKinect_REMOVEUSER(self, sensor):
		# Last one out, hit the lights.
		if not sensor.users:
			self.floorplan.get_servo('BedLightStrip').turn_off_full()
	
	def event_BedroomKinect_POSITION(self, sensor, pos):
		#self.floorplan.get_servo('BedLightStrip').send_test_message()
		pass

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
	def m(s):
		feet = 0.305 # m
		inch = feet / 12. # m
		matches = re.match(r'(-?\d+)\'(?:(-?\d+)")?', s.strip())
		if matches:
			if len(matches.groups()) == 2:
				return int(matches.group(1)) * feet
			return int(matches.group(1)) * feet + int(matches.group(2)) * inch

	flat = FloorPlan('TrainedMonkeyStudios')
	rules = HouseRules(flat)

	# Add all logical rooms.
	bathroom = flat.add_room('Bathroom', m(''' 11'3" '''),  m(''' 4'9" '''))
	bedroom = flat.add_room( 'Bedroom',  m(''' 12'4" '''),  m(''' 12'7" '''))
	pantry = flat.add_room(  'Pantry',   m(''' 4'7" '''),   m(''' 2'6" '''))
	laundry = flat.add_room( 'Laundry',  m(''' 3'1" '''),   m(''' 2'6" '''))
	hallway = flat.add_room( 'Hallway',  m(''' 3' '''),     m(''' 7'7" '''))
	kitchen = flat.add_room( 'Kitchen',  m(''' 8'4" '''),   m(''' 8' '''))
	entry = flat.add_room(   'Entry',    m(''' 3'8" '''),   m(''' 11'11" '''))
	living = flat.add_room(  'Living',   m(''' 12'4"  '''), m(''' 11'11"  '''))
	porch = flat.add_room(   'Porch',    m(''' 6'7" '''),   m(''' 11'11" '''))
	dining = flat.add_room(  'Dining',   m(''' 7'7" '''),   m(''' 8'3" '''))
	
	# Connect our rooms.
	doors = [
		(('''2'4"''', '''6"'''), 'Bathroom', ('''8'11"''', '''4'6"'''), 'Hallway', ('''0'4"''', '''0'-3"''')), 
		(('''6"''', '''2'6"'''), 'Bedroom', ('''3"''', '''9'10"'''), 'Hallway', ('''2'9"''', '''6'10"''')),
	]
	for size, name1, pos1, name2, pos2 in doors:
		room1 = flat.get_room(name1)
		room2 = flat.get_room(name2)
		size = (m(size[0]), m(size[1]))
		pos1 = (m(pos1[0]), m(pos1[1]))
		pos2 = (m(pos2[0]), m(pos2[1]))
		room1.add_portal_to(room2, size, pos1)
		room2.add_portal_to(room1, size, pos2)
	
	# Add all the sensors.
	sensors = [
		(Kinect, 'BedroomKinect', '127.0.0.1', Network.DefaultSensorPort,
				[('Bedroom', m(''' 12'4" '''), m(''' 12'7" '''), m(''' 6' '''))])
	]
	for cls, name, host, port, rooms in sensors:
		s = cls(rules, name, (host, port))
		for roomName, X, Y, Z in rooms:
			flat.add_sensor(s, roomName, (X, Y, Z))

	# Add all servos.
	servos = [
		(LightStrip, 'BedLightStrip', '127.0.0.1', Network.DefaultServoPort, 'Bedroom')
	]
	for cls, name, host, port, room in servos:
		s = cls(name, (host, port))
		flat.add_servo
	bedLightStrip = LightStrip('BedLightStrip', '127.0.0.1')
	flat.add_servo(bedLightStrip, 'Bedroom')

	return flat

def main():
	floorplan = build_floorplan()

	network = Network(floorplan)

	return network.run()


if __name__ == '__main__':
	sys.exit(main())
