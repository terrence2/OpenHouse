# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp.actuators import Actuator
from mcp.color import BHS, RGB, Mired, Color

from threading import Thread, Lock
from queue import Queue
from pprint import pprint

import colorsys
import logging
import http
import http.client
import json

log = logging.getLogger('hue')


class LightState:
    """
    Current state of a given hue light.
    """
    def __init__(self, light_id: str, state: {}):
        self.light_id = light_id

        self.modelid = state['modelid']
        self.swversion = state['swversion']

        self.on = state['state']['on']
        self.colormode = state['state']['colormode']
        self.hue = state['state']['hue']
        self.bri = state['state']['bri']
        self.sat = state['state']['sat']
        self.ct = state['state']['ct']

    def update_from_response(self, property_name: str, property_value: str):
        log.debug('updating hue light state from response: @id{}: {} <- {}'.format(
            self.light_id, property_name, property_value))

        assert property_name in self.__dict__
        setattr(self, property_name, property_value)
        if property_name == 'ct':
            self.colormode = 'ct'
        elif property_name == 'bri':
            self.colormode = 'hs'


class HueBridge(Thread):
    """
    A Philips Hue bridge which provides access to individual Hue lights.

    Since lights are only accessible behind a bridge, this class makes common
    the bridge attributes shared by multiple HueLight instances. It should be
    constructed and passed to HueLights, but generally not used directly.
    """
    def __init__(self, address: str, username: str, lock: Lock):
        super().__init__()
        self.setDaemon(True)

        self.lock_ = lock
        self.queue_ = Queue()

        self.address = address
        self.username = username

        # Mapping between light names and light id's so we don't have to enter them manually.
        self.known_lights = {}  # {name:str => id:str}

        # Current light values.
        self.light_values = {}  # {id:str => LightState}

        lights = self._make_request('GET', '/lights')
        for id_str, light_def in lights.items():
            self.known_lights[light_def['name']] = id_str
            self.light_values[id_str] = LightState(id_str, light_def)

    def get_state(self, id_str: str) -> LightState:
        return self.light_values[id_str]

    def identify_light(self, name: str) -> str:
        return self.known_lights[name]

    def request(self, mode: str, resource: str, data: {}=None):
        self.queue_.put((mode, resource, data))

    def parse_url_to_light_and_property(self, url: str) -> str:
        assert url[0] == '/'
        parts = url[1:].split('/')
        assert parts[0] == 'lights'
        id_str = parts[1]
        assert parts[2] == 'state'
        prop_name = parts[3]
        state = self.get_state(id_str)
        return state, prop_name

    def run(self):
        while True:
            mode, resource, data = self.queue_.get()
            log.warning("Got request: {}, {}".format(mode, resource))
            results = self._make_request(mode, resource, data)

            # Update light state from response, rather than re-querying.
            for result in results:
                if 'success' in result:
                    for light_url, property_value in result['success'].items():
                        light_state, property_name = self.parse_url_to_light_and_property(light_url)
                        light_state.update_from_response(property_name, property_value)

    def _make_request(self, mode: str, resource: str, data: {}=None) -> {}:
        if data is not None:
            data = json.dumps(data).encode('UTF-8')
        conn = http.client.HTTPConnection(self.address)
        conn.request(mode, '/api/' + self.username + resource, data)
        res = conn.getresponse()
        result_bytes = res.read()
        conn.close()
        result = json.loads(str(result_bytes, encoding='UTF-8'))
        log.debug('{} {} :: {} -> {}'.format(mode, resource, data, result))
        return result


class HueLight(Actuator):
    """
    An individually controllable Philips Hue light.
    """
    def __init__(self, name: str, bridge: HueBridge):
        super().__init__(name)
        self.hue_bridge = bridge
        self.hue_light_id = bridge.identify_light(name)

        state = self.hue_bridge.get_state(self.hue_light_id)
        log.info('HueLight(name="{}", id="{}", model="{}", swversion="{}"'.format(name, self.hue_light_id, state.modelid, state.swversion))

    def set(self, **args):
        # Parse arguments into a set of request properties for the hue bridge.
        request_properties = {}
        for prop_name, prop_value in args.items():
            if prop_name == 'on' and self.on != prop_value:
                request_properties['on'] = bool(prop_value)
            elif prop_name == 'color' and self.color != prop_value:
                request_properties.update(self.color_to_request(prop_value))

        if not request_properties:
            log.warning("skipping HueLight.set because of empty request for args {}".format(args))
            return

        url = "/lights/{}/state".format(self.hue_light_id)
        self.hue_bridge.request("PUT", url, request_properties)

    @classmethod
    def color_to_request(cls, color: Color):
        """Add the properties from Color to a json object suitable for passing to the hue API."""
        if isinstance(color, BHS):
            return {'bri': color.b, 'hue': color.h, 'sat': color.s}
        elif isinstance(color, Mired):
            return {'ct': color.ct}
        assert isinstance(color, RGB)
        bhs = BHS.from_rgb(color)
        return {'bri': bhs.b, 'hue': bhs.h, 'sat': bhs.s}

    @property
    def modelid(self) -> str:
        return self.hue_bridge.get_state(self.hue_light_id).modelid

    @property
    def swversion(self) -> str:
        return self.hue_bridge.get_state(self.hue_light_id).swversion

    @property
    def on(self) -> bool:
        return self.hue_bridge.get_state(self.hue_light_id).on

    @property
    def color(self) -> Color:
        state = self.hue_bridge.get_state(self.hue_light_id)
        if state.colormode == 'hs':
            return BHS(state.bri, state.hue, state.sat)
        assert state.colormode == 'ct'
        return Mired(state.ct)


