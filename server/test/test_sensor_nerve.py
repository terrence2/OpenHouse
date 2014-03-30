__author__ = 'terrence'

from unittest import TestCase
import time
import threading

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

    def publish_motion(self, state):
        self.socket.send_json({'type': 'MOVEMENT', 'state': state})


class TestNerve(TestCase):
    def setUp(self):
        pass

    def tearDown(self):
        pass

    def test_read_weather(self):
        have_temperature = 0
        def receive_temperature(expect_temperature: [int]):
            nonlocal have_temperature
            def callback(event):
                nonlocal have_temperature
                self.assertEqual("temperature", event.name)
                self.assertEqual(expect_temperature[have_temperature], event.value)
                have_temperature += 1
            return callback

        have_humidity = 0
        def receive_humidity(expect_humidity: [int]):
            nonlocal have_humidity
            def callback(event):
                nonlocal have_humidity
                self.assertEqual("humidity", event.name)
                self.assertEqual(expect_humidity[have_humidity], event.value)
                have_humidity += 1
            return callback

        have_motion = 0
        def receive_motion(expect_motion: [int]):
            nonlocal have_motion
            def callback(event):
                nonlocal have_motion
                self.assertEqual("motion", event.name)
                self.assertEqual(expect_motion[have_motion], event.value)
                have_motion += 1
            return callback

        remote = FakeNerve()
        local = nerve.Nerve('TestNerve', ('127.0.0.1', network.Bus.DefaultSensorPort))
        local.listen_temperature(receive_temperature([10, 20, 30]))
        local.listen_humidity(receive_humidity([50, 60, 70]))
        local.listen_motion(receive_motion([True, False, True, False]))

        bus = network.Bus(threading.Lock())
        bus.add_sensor(local.remote)
        bus.start()

        # Pump the first event until we start receiving events.
        remote.publish_weather(10, 50)
        time.sleep(0.01)
        while have_temperature < 1 and have_humidity < 1:
            remote.publish_weather(10, 50)
            time.sleep(0.01)

        remote.publish_weather(20, 60)
        remote.publish_weather(30, 70)

        remote.publish_motion(True)
        remote.publish_motion(False)
        remote.publish_motion(True)
        remote.publish_motion(False)

        bus.exit()
        bus.join()

