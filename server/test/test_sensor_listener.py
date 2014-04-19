from unittest import TestCase
import threading
import time
import zmq

from mcp import network
from mcp.sensors.listener import Listener, ListenerEvent

__author__ = 'terrence'


class FakeListener:
    def __init__(self):
        self.ctx = zmq.Context()
        self.socket = self.ctx.socket(zmq.PUB)
        self.socket.bind("tcp://*:" + str(network.Bus.DefaultSensorPort))

    def publish_command(self, command):
        self.socket.send_json({'command': command})


class TestListener(TestCase):
    def test_listen_for_commands(self):
        received_commands = []
        def receive_command():
            nonlocal received_commands
            def callback(event: ListenerEvent):
                nonlocal received_commands
                received_commands.append(event.command)
            return callback

        remote = FakeListener()
        local = Listener('TestListener', ('127.0.0.1', network.Bus.DefaultSensorPort))
        local.listen_for_commands(receive_command())

        bus = network.Bus(threading.Lock())
        bus.add_sensor(local.remote)
        bus.start()

        # Pump the first event until we start receiving events.
        remote.publish_command("FIRST")
        time.sleep(0.01)
        while len(received_commands) < 1:
            remote.publish_command("FIRST")
            time.sleep(0.01)

        remote.publish_command("SECOND")
        remote.publish_command("THIRD")
        remote.publish_command("FOURTH")
        remote.publish_command("FIFTH")

        while len(received_commands) < 5:
            time.sleep(0.01)

        bus.exit()
        bus.join()

        self.assertSequenceEqual(received_commands, ["FIRST", "SECOND", "THIRD", "FOURTH", "FIFTH"])

