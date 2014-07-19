# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

import zmq

from mcp import network
from mcp.sensors import Sensor, MotionEvent, SwitchEvent

log = logging.getLogger('wemo-sensor')


class WeMoSensorBridge:
    """
    Interfaces with the zmq bridge to get the insane UPnP, Python2 world the wemo lives in.
    """
    ReplyPort = 31978
    BroadcastPort = 31979

    def __init__(self, address_without_port: str):
        self.address_without_port = address_without_port

        # When we need to get a device's state, we want to do so synchronously.
        # Make a separate connection for our own sync use in addition to the
        # one made by network.Bus.
        self.ctx = zmq.Context()
        self.request_socket = self.ctx.socket(zmq.REQ)
        req_address = "tcp://{}:{}".format(self.address_without_port, self.ReplyPort)
        log.debug("connecting for sync reply at: {}".format(req_address))
        self.request_socket.connect(req_address)

        # All sensor messages arrive via the bridge, so we need to process the messages
        # on the bridge and dispatch from there. Otherwise, each WeMoDevice has to get
        # every message from the network and do its own filtering. Instead we just attach
        # the bridge and have it do dispatch.
        self.devices = {}  # {name: WeMoMotion}
        self.name = self.address_without_port
        self.address = (self.address_without_port, self.BroadcastPort)
        self.remote = network.Sensor(self)

    def on_message(self, wrapper: object):
        source = wrapper['source']
        message = wrapper['message']
        log.debug("got message for {}: {}".format(source, message))
        try:
            self.devices[source].on_message(message)
        except KeyError:
            log.warning("unknown device: {}".format(source))

    def add_device(self, device: Sensor) -> Sensor:
        self.devices[device.name] = device
        return device

    def get_state(self, name: str) -> bool:
        log.debug("getting state for {}".format(name))
        self.request_socket.send_json({'target': name, 'type': 'get_state'})
        data = self.request_socket.recv_json()
        log.debug("state of {} is {}".format(name, data['state']))
        return bool(data['state'])


class WeMoMotion(Sensor):
    def __init__(self, hostname: str, bridge: WeMoSensorBridge):
        super().__init__(hostname)
        self.bridge_ = bridge
        self.motion_listener_ = self.default_motion_listener_

    def default_motion_listener_(self, event: MotionEvent):
        log.warning('ignoring motion from {}: {}'.format(self.name, event.value))

    def get_state(self) -> bool:
        return self.bridge_.get_state(self.name)

    def listen_motion(self, listener: callable):
        self.motion_listener_ = listener

    def on_message(self, message: object):
        self.motion_listener_(MotionEvent(bool(message['state'])))


class WeMoSwitch(Sensor):
    """
    The sensor half of the switch. Receives updates on the switch's state.
    """
    def __init__(self, hostname: str, bridge: WeMoSensorBridge):
        super().__init__(hostname)
        self.bridge_ = bridge
        self.state_listener_ = self.default_switch_listener_

    def default_switch_listener_(self, event: SwitchEvent):
        log.warning('ignoring motion from {}: {}'.format(self.name, event.value))

    def get_state(self) -> bool:
        return self.bridge_.get_state(self.name)

    def listen_switch_state(self, listener: callable):
        self.state_listener_ = listener

    def on_message(self, message: object):
        self.state_listener_(SwitchEvent(bool(message['state'])))

