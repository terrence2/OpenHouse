# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
try:
    from enum import Enum
except ImportError:
    class Enum:
        pass

import logging
import os
import socket

from select import POLLIN
from threading import Thread
from queue import Queue, Empty

import zmq
from zmq.sugar import socket as zmq_socket

log = logging.getLogger('network')


def get_own_internal_ip_slow():
    """
    Discovering the active internal interface that new connections will get spawned on -- e.g. that local peers can
    (in typical networks) call back on -- is actually quite hard. We spawn a connection to an external resource and
    derive the internal network from that. A rather inelegant hack, but it gets the job done.
    """
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(('8.8.8.8', 80))
        return s.getsockname()[0]
    except socket.error:
        return None
    finally:
        # Don't wait around for the GC.
        s.close()
        del s


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
        self.queue_ = Queue()
        self.ready_to_send = True

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
        self.queue_.put(message)
        self.bus.poke()

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

        # The poke socket.
        self.read_fd_, self.write_fd_ = os.pipe()
        self.poller.register(self.read_fd_, POLLIN)

        # Derive the internal ip address as part of our interface.
        self.internal_address = get_own_internal_ip_slow()

    def cleanup(self):
        for socket in self.actuators:
            socket.close()
        for socket in self.sensors:
            socket.close()
        os.close(self.read_fd_)
        os.close(self.write_fd_)

    def connect_(self, address: (str, int), socket_type: "zmq socket type"):
        socket = self.ctx.socket(socket_type)
        address = "tcp://" + str(address[0]) + ":" + str(address[1])
        log.info("Connecting to sensor at: {}".format(address))
        socket.connect(address)
        self.poller.register(socket, POLLIN)
        return socket

    def add_actuator(self, actuator: Actuator):
        assert not hasattr(actuator, 'remote')
        actuator.bus = self
        actuator.socket = self.connect_(actuator.address, zmq.REQ)
        self.actuators[actuator.socket] = actuator

    def add_sensor(self, sensor: Sensor):
        assert not hasattr(sensor, 'remote')
        sensor.bus = self
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
                continue

            for (socket, event) in ready:
                self.check_outgoing_messages_(socket, event)
                self.check_sensors_(socket, event)
                self.check_actuators_(socket, event)

        self.cleanup()

    def poke(self):
        os.write(self.write_fd_, b'1')

    def exit(self):
        self.ready_to_exit = True
        self.poke()

    def check_outgoing_messages_(self, socket, event: int):
        if socket != self.read_fd_:
            # Not a poke event, skip.
            return

        if event != POLLIN:
            log.warning("unknown error on poke fd")
            return

        # Clear the buffered poke byte.
        buf = os.read(self.read_fd_, 4096)
        assert buf == b'1'

        for actuator in self.actuators.values():
            if not actuator.queue_.empty() and actuator.ready_to_send:
                log.info("Sending message to actuator: {}".format(actuator.name))
                actuator.socket.send_json(actuator.queue_.get_nowait())
                actuator.ready_to_send = False

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
            return

        log.debug("Received message from sensor: {}".format(sensor.name))

        try:
            data = socket.recv_json()
        except Exception:
            log.exception("failed to receive sensor message")
            return

        try:
            with self.lock_:
                sensor.on_message(data)
        except Exception:
            log.exception("failed to handle sensor message")
            return

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
            return

        log.info("Received message from actuator: {}".format(actuator.name))

        try:
            data = socket.recv_json()
        except Exception:
            log.exception("failed to receive sensor message")
            return
        actuator.ready_to_send = True

        try:
            with self.lock_:
                actuator.on_reply(data)
        except Exception:
            log.exception("failed to handle actuator reply")
            return

        # Check for any messages waiting to send.
        try:
            to_send = actuator.queue_.get_nowait()
            actuator.socket.send_json(to_send)
            actuator.ready_to_send = False
        except Empty:
            return


