__author__ = 'terrence'

from unittest import TestCase
import time

import zmq

from mcp import network
from mcp.sensors import nerve


class FakeNerve:
    def __init__(self):
        self.ctx = zmq.Context()
        self.socket = self.ctx.socket(zmq.PUB)
        self.socket.bind("tcp://*:" + str(network.Bus.DefaultSensorPort))

    def publish_weather(self, temp, humidity):
        self.socket.send_json({'type': 'TEMP_HUMIDITY', 'temp': temp, 'humidity': humidity})

    def publish_movement(self, state):
        self.socket.send_json({'type': 'MOVEMENT', 'state': state})


class TestNerve(TestCase):
    """
    def setUp(self):
        self.remote = FakeNerve()

        self.local = nerve.Nerve(None, )

        self.bus = network.Bus()
        self.bus.add_sensor(local.remote)
        self.bus.start()

    def tearDown(self):
        self.bus.exit()
        self.bus.join()

    def test_read_weather(self):
        self.remote.publish_weather(10, 50)
        time.sleep(0.1)
        self.assertEqual(self.local.last_temperature, 10)
        self.assertEqual(self.local.last_humidity, 50)

    def test_read_motion(self):
        self.remote.publish_movement(True)
        time.sleep(0.1)
        self.assertEqual(self.local.last_motion_state, True)

        self.remote.publish_movement(False)
        time.sleep(0.1)
        self.assertEqual(self.local.last_motion_state, False)
    """
