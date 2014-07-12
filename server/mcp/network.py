# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
try:
    from enum import Enum
except ImportError:
    class Enum:
        pass

import logging
import zmq
from zmq.sugar import socket as zmq_socket
from select import POLLIN
from threading import Thread

log = logging.getLogger('network')


class DeviceType(Enum):
    sensor = 1
    actuator = 2


class Sensor:
    def __init__(self, sensor: object):
        """
        Construct a network sensor given some object with a name and address.
        Forwards messages on the network at |address| to the given |sensor|
        instance.
        """
        assert hasattr(sensor, 'name')
        assert hasattr(sensor, 'address')
        assert hasattr(sensor, 'on_message')
        self.instance = sensor
        self.socket = None  # ZMQ socket, assigned by Bus.

    @staticmethod
    def device_type():
        return DeviceType.sensor

    @property
    def name(self):
        return self.instance.name

    @property
    def address(self):
        return self.instance.address

    def on_message(self, message: object):
        self.instance.on_message(message)


class Actuator:
    def __init__(self, actuator: object):
        """
        Construct a network actuator given some object with name and address.
        """
        self.bus = None
        self.socket = None
        self.instance = actuator

    @staticmethod
    def device_type():
        return DeviceType.actuator

    @property
    def name(self):
        return self.instance.name

    @property
    def address(self):
        return self.instance.address

    def send_message(self, message: object):
        assert self.socket is not None
        self.socket.send_json(message)

    def on_reply(self, message: object):
        self.instance.on_reply(message)


class Bus(Thread):
    """
    The global message bus for an MCP instance. Receives sensor traffic from
    the network and forwards it to the connected model for further processing.
    """

    DefaultSensorPort = 31975
    DefaultActuatorPort = 31978
    Interval = 500

    def __init__(self, lock):
        super().__init__()
        self.ready_to_exit = False

        # A lock to hold while executing response code.
        self.lock_ = lock

        self.ctx = zmq.Context()
        self.poller = zmq.Poller()
        self.sensors = {}  # {zmq.socket: Sensor}
        self.actuators = {}  # {zmq.socket: Actuator}

    def connect_(self, address: (str, int), socket_type: "zmq socket type"):
        socket = self.ctx.socket(socket_type)
        address = "tcp://" + str(address[0]) + ":" + str(address[1])
        log.info("Connecting to sensor at: {}".format(address))
        socket.connect(address)
        self.poller.register(socket, POLLIN)
        return socket

    def add_actuator(self, actuator: Actuator):
        assert not hasattr(actuator, 'remote')
        actuator.socket = self.connect_(actuator.address, zmq.REQ)
        self.actuators[actuator.socket] = actuator

    def add_sensor(self, sensor: Sensor):
        assert not hasattr(sensor, 'remote')
        sensor.socket = self.connect_(sensor.address, zmq.SUB)
        sensor.socket.setsockopt(zmq.SUBSCRIBE, b'')
        self.sensors[sensor.socket] = sensor

    def add_device(self, device: Sensor or Actuator):
        if device.device_type() == DeviceType.sensor:
            return self.add_sensor(device)
        assert device.device_type() == DeviceType.actuator
        return self.add_actuator(device)

    def run(self):
        while not self.ready_to_exit:
            ready = self.poller.poll(Bus.Interval)
            if not ready:
                # self.model.handle_timeout()
                continue

            for (socket, event) in ready:
                self.check_sensors_(socket, event)
                self.check_actuators_(socket, event)

        for socket in self.actuators:
            socket.close()
        for socket in self.sensors:
            socket.close()

    def exit(self):
        self.ready_to_exit = True

    def check_sensors_(self, socket: zmq_socket, event: int):
        if event != POLLIN:
            log.warning("unknown error on sensor socket")
            return

        if socket not in self.sensors:
            return

        try:
            sensor = self.sensors[socket]
        except KeyError:
            log.exception("received message from unknown sensor")

        log.debug("Received message from sensor: {}".format(sensor.name))

        try:
            data = socket.recv_json()
        except Exception:
            log.exception("failed to receive sensor message")

        try:
            with self.lock_:
                sensor.on_message(data)
        except Exception:
            log.exception("failed to handle sensor message")

    def check_actuators_(self, socket: zmq_socket, event: int):
        if event != POLLIN:
            log.warning("unknown error on actuator socket")
            return

        if socket not in self.actuators:
            return

        try:
            actuator = self.actuators[socket]
        except KeyError as ex:
            log.exception("received message from unknown actuator")

        log.info("Received message from actuator: {}".format(actuator.name))

        try:
            data = socket.recv_json()
        except Exception:
            log.exception("failed to receive sensor message")

        try:
            with self.lock_:
                actuator.on_reply(data)
        except Exception:
            log.exception("failed to handle actuator reply")

