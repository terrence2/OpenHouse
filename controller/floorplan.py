from sensors import Sensor
from servos import Servo

class Portal:
	def __init__(self, target, width, height, x, y):
		self.target = target
		self.width = width # float in meters
		self.height = height
		self.x = x
		self.y = y

class Room:
	"""
	An axis-aligned rectangular extent in a floorplan.
	"""
	def __init__(self, name, width, height):
		super().__init__()
		self.name = name
		self.width = width
		self.height = height

		self.portals = []
		self.sensors = {}
		self.servos = {}

	def add_portal_to(self, other, size, position):
		p = Portal(other, size[0], size[1], position[0], position[1])
		self.portals.append(p)
		return p

	def add_servo(self, servo:Servo):
		assert servo.name not in self.servos
		self.servos[servo.name] = servo

	def add_sensor(self, sensor:Sensor, position:(float, float)):
		assert sensor.name not in self.sensors
		self.sensors[sensor.name] = {'position': position, 'sensor': sensor}

class FloorPlan:
	"""
	Contains Rooms filled with Sensors and Servos and links them together into
	a conceptual space.
	"""
	def __init__(self, name):
		super().__init__()
		self.name = name
		self.rooms = {}
		self.sensors = {}
		self.servos = {}
	
	def add_room(self, name, width, height) -> Room:
		assert name not in self.rooms
		self.rooms[name] = Room(name, width, height)
		return self.rooms[name]

	def get_room(self, name:str) -> Room:
		return self.rooms[name]

	def add_servo(self, servo:Servo, roomName:str):
		if servo.name not in self.servos:
			self.servos[servo.name] = servo
		assert servo is self.servos[servo.name]
		self.rooms[roomName].add_servo(servo)

	def get_servo(self, name:str):
		return self.servos[name]

	def all_servos(self):
		return self.servos.values()

	def add_sensor(self, sensor:Sensor, roomName:str, position:(float,float)):
		if sensor.name not in self.sensors:
			self.sensors[sensor.name] = sensor
		assert sensor is self.sensors[sensor.name]
		self.rooms[roomName].add_sensor(sensor, position)
	
	def get_sensor(self, name:str):
		return self.sensors[name]

	def all_sensors(self):
		return self.sensors.values()

	def handle_sensor_message(self, json):
		if 'name' not in json:
			print("Dropping invalid message: no name")
			return

		name = json['name']
		if name not in self.sensors:
			print("Got control message from unknown sensor: {}".format(name))
			return

		sensor = self.sensors[name]
		sensor.handle_sensor_message(json)

