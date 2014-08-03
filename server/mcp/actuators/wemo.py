# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

import zmq

from mcp import network
from mcp.actuators import Actuator

log = logging.getLogger('wemo-actuator')


class WeMoActuatorBridge:
    ReplyPort = 31978

    def __init__(self, address_without_port: str):
        self.address = (address_without_port, self.ReplyPort)

        # When we need to get a device's state, we want to do so synchronously.
        # Make a separate connection for our own sync use in addition to the
        # one made by network.Bus.
        self.ctx = zmq.Context()
        self.request_socket = self.ctx.socket(zmq.REQ)
        req_address = "tcp://{}:{}".format(address_without_port, self.ReplyPort)
        log.debug("connecting for sync reply at: {}".format(req_address))
        self.request_socket.connect(req_address)

    def get_state(self, name: str) -> bool:
        log.debug("getting state for {}".format(name))
        self.request_socket.send_json({'target': name, 'type': 'get_state'})
        data = self.request_socket.recv_json()
        log.debug("state of {} is {}".format(name, data['state']))
        return bool(data['state'])

    def set_group(self, devices: [Actuator], properties: {}):
        # FIXME: find a way to send the full set of sets to the bridge in one message.
        for device in devices:
            for prop_name, prop_value in properties.items():
                setattr(device, prop_name, prop_value)


class WeMoSwitch(Actuator):
    def __init__(self, name: str, bridge: WeMoActuatorBridge):
        super().__init__(name)
        self.bridge = bridge
        self.address = self.bridge.address
        self.remote = network.Actuator(self)

    @property
    def on(self) -> bool:
        return self.bridge.get_state(self.name)

    @on.setter
    def on(self, state: bool):
        self.remote.send_message({'target': self.name, 'type': 'set_state', 'state': state})


