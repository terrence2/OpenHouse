from network import Network

class Servo:
	def __init__(self, name:str, addr:(str,int)):
		super().__init__()
		self.name = name

		# Currently unused, as we simply bcast on the servo socket and servos
		# subscribe to get updates.
		self.addr = addr

		# The socket we need to broadcast on to reach our servo.
		self.socket = None

	def set_socket(self, sock):
		self.socket = sock

	def send_message(self, json):
		print("Sending on socket: {}".format(json))
		self.socket.send_json(json)

class LightStrip(Servo):
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)
		self.state = 's'
		
	def turn_on_full(self):
		self.send_message({
			'name': self.name,
			'type': 'ON'
		})

	def turn_off_full(self):
		self.send_message({
			'name': self.name,
			'type': 'OFF'
		})
	
	"""
	def send_test_message(self):
		self.send_message({
			'name': self.name,
			'type': 'TEST'
		})
	"""
