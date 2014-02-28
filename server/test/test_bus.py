from unittest import TestCase
from mcp import network
import time
import zmq

__author__ = 'terrence'

class FakeActuator:
    def __init__(self):
        self.ctx = zmq.Context()
        self.socket = self.ctx.socket(zmq.REP)
        self.socket.bind("tcp://*:" + str(network.Bus.DefaultActuatorPort))
        self.request = None

    def wait_for_message(self):
        assert self.request is None
        self.request = self.socket.recv_json()
        self.socket.send_json({'hi': 'actuator'})
        self.socket.close()


class LocalActuator:
    def __init__(self):
        self.name = "TestActuator"
        self.address = ("127.0.0.1", network.Bus.DefaultActuatorPort)
        self.remote = None
        self.reply = None

    def actuate(self, message: object):
        assert self.remote is not None
        assert self.reply is None
        self.remote.send_message(message)

    def on_reply(self, reply: object):
        self.reply = reply

    def wait_for_reply(self):
        while self.reply is None:
            time.sleep(0.1)


class FakeSensor:
    def __init__(self):
        self.ctx = zmq.Context()
        self.socket = self.ctx.socket(zmq.PUB)
        self.socket.bind("tcp://*:" + str(network.Bus.DefaultSensorPort))

    def publish(self):
        self.socket.send_json({'some': 'data'})


class LocalSensor:
    def __init__(self):
        self.name = "TestSensor"
        self.address = ("127.0.0.1", network.Bus.DefaultSensorPort)
        self.message = None

    def on_message(self, message):
        self.message = message


class TestBus(TestCase):
    def test_add_actuator(self):
        remote = FakeActuator()

        local = LocalActuator()
        local.remote = network.Actuator(local)

        bus = network.Bus()
        bus.add_actuator(local.remote)
        bus.start()

        # Dispatch an action and make sure it arrives at actuator.
        local.actuate({'hello': 'world'})
        remote.wait_for_message()
        self.assertEqual(remote.request, {'hello': 'world'})

        # Wait for the actuator's response.
        local.wait_for_reply()
        self.assertEqual(local.reply, {'hi': 'actuator'})

        bus.exit()
        bus.join()

    def test_add_sensor(self):
        remote = FakeSensor()

        local = LocalSensor()
        local.remote = network.Sensor(local)

        bus = network.Bus()
        bus.add_sensor(local.remote)
        bus.start()

        while local.message is None:
            remote.publish()
            time.sleep(0.1)
        self.assertEqual(local.message, {'some': 'data'})

        bus.exit()
        bus.join()

    def test_run(self):
        bus = network.Bus()
        bus.start()
        bus.exit()
        bus.join()

