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
        self.on_temperature_ = None
        self.on_humidity_ = None
        self.on_motion_ = None

        #return str(self.last_temperature * 9.0 / 5.0 + 32.0) + "\n"

    def listen_temperature(self, callback: callable):
        assert self.on_temperature_ is None
        self.on_temperature_ = callback

    def listen_humidity(self, callback: callable):
        assert self.on_humidity_ is None
        self.on_humidity_ = callback

    def listen_motion(self, callback: callable):
        assert self.on_motion_ is None
        self.on_motion_ = callback

    def on_message(self, json):
        """
        Called by the sensor model to inform us of new messages from the network.
        """
        msg_type = json['type']
        if msg_type == 'TEMP_HUMIDITY':
            temp, humidity = float(json['temp']), float(json['humidity'])
            log.info("from {} -> temperature: {}, humidity: {}".format(self.name, temp, humidity))
            if self.on_temperature_:
                self.on_temperature_(NerveEvent('temperature', temp))
            if self.on_humidity_:
                self.on_humidity_(NerveEvent('humidity', humidity))

        elif msg_type == 'MOVEMENT':
            state = bool(json['state'])
            log.info("from {} -> motion state: {}".format(self.name, state))
            if self.on_motion_:
                self.on_motion_(NerveEvent('motion', bool(json['state'])))

        else:
            log.error("Unrecognized message type from Nerve {}: {}".format(self.name, msg_type))


