__author__ = 'terrence'

import logging

from mcp.sensors import Sensor
from mcp import network

log = logging.getLogger('listener')


class ListenerEvent:
    def __init__(self, sensor, command):
        self.sensor = sensor
        self.command = command


class Listener(Sensor):
    def __init__(self, name: str, address: (str, int)):
        super().__init__()

        # The bus protocol requires these properties:
        self.name = name
        self.address = address
        self.remote = network.Sensor(self)

        # Callbacks for the events we can send.
        self.on_command_ = None

    def listen_for_commands(self, callback: callable):
        assert self.on_command_ is None
        self.on_command_ = callback

    def on_message(self, json):
        """
        Called by the sensor model to inform us of new messages from the network.
        """
        log.info("Received message: {}".format(json))
        self.on_command_(ListenerEvent(self, json['command']))