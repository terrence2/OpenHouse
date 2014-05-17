# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import logging
from mcp import network
from mcp.sensors import Sensor

log = logging.getLogger('nerve')


class NerveEvent:
    def __init__(self, name, value):
        self.name = name
        self.value = value


class Nerve(Sensor):
    def __init__(self, name: str, address: (str, int)):
        super().__init__(name)

        # The bus protocol requires these properties:
        self.address = address
        self.remote = network.Sensor(self)

        # Callbacks for the events we can send.
        self.on_temperature_ = self.fake_listener_
        self.on_humidity_ = self.fake_listener_
        self.on_motion_ = self.fake_listener_

    def fake_listener_(self, evt: NerveEvent):
        log.warning("nerve {} dropping event {}".format(self.name, evt.name))

    def listen_temperature(self, callback: callable):
        self.on_temperature_ = callback

    def listen_humidity(self, callback: callable):
        self.on_humidity_ = callback

    def listen_motion(self, callback: callable):
        self.on_motion_ = callback

    def on_message(self, json):
        """
        Called by the sensor model to inform us of new messages from the network.
        """
        msg_type = json['type']
        if msg_type == 'TEMP_HUMIDITY':
            temp, humidity = float(json['temp']), float(json['humidity'])
            log.debug("from {} -> temperature: {}, humidity: {}".format(self.name, temp, humidity))
            if self.on_temperature_:
                self.on_temperature_(NerveEvent('temperature', temp))
            if self.on_humidity_:
                self.on_humidity_(NerveEvent('humidity', humidity))

        elif msg_type == 'MOVEMENT':
            state = bool(json['state'])
            log.debug("from {} -> motion state: {}".format(self.name, state))
            if self.on_motion_:
                self.on_motion_(NerveEvent('motion', bool(json['state'])))

        else:
            log.error("Unrecognized message type from Nerve {}: {}".format(self.name, msg_type))


