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


class HueLightGroup:
    def __init__(self, group_id: int, lights: list):
        self.group_id = group_id
        self.lights = set(lights)

    def is_same_as(self, lights: list):
        return set(lights) == self.lights


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

        # List of known groups.
        self.groups = []
        groups = self._make_request('GET', '/groups')
        log.info("GET /groups: {}".format(groups))

        # Mapping between light names and light id's so we don't have to enter them manually.
        self.known_lights = {}  # {name:str => id:str}

        # Current light values.
        self.light_values = {}  # {id:str => LightState}

        slash = self._make_request('GET', '/')
        log.info("GET /: {}".format(slash))

        lights = self._make_request('GET', '/lights')
        for id_str, light_def in lights.items():
            self.known_lights[light_def['name']] = id_str
            self.light_values[id_str] = LightState(id_str, light_def)

        groups = self._make_request('GET', '/groups')
        log.info("GET /groups: {}".format(groups))

    def get_state(self, id_str: str) -> LightState:
        return self.light_values[id_str]

    def identify_light(self, name: str) -> str:
        return self.known_lights[name]

    def parse_url_to_lights_and_property(self, url: str) -> ([LightState], str):
        assert url[0] == '/'
        parts = url[1:].split('/')
        if parts[0] == 'lights':
            id_str = parts[1]
            assert parts[2] == 'state'
            prop_name = parts[3]
            state = self.get_state(id_str)
            return [state], prop_name
        assert parts[0] == 'groups'
        group_id = int(parts[1])
        group = self.groups[group_id]
        states = [self.get_state(light.hue_light_id) for light in group.lights]
        assert parts[2] == 'action'
        prop_name = parts[3]
        return states, prop_name

    def handle_success_result(self, result: {}):
        for light_url, property_value in result.items():
            lights, property_name = self.parse_url_to_lights_and_property(light_url)
            for light in lights:
                light.update_from_response(property_name, property_value)

    def run(self):
        while True:
            mode, resource, data = self.queue_.get()
            results = self._make_request(mode, resource, data)

            # Update light state from response, rather than re-querying.
            for result in results:
                if 'success' in result:
                    self.handle_success_result(result['success'])

    def request(self, mode: str, resource: str, data: {}=None):
        self.queue_.put((mode, resource, data))

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

    def set_properties_on_all_devices(self, devices, kwargs):
        # Attempt to find a group we can set in one go.
        for group in self.groups:
            if group.is_same_as(devices):
                url = "/groups/{}/action".format(group.group_id)
                request_props = self.kwargs_to_json(kwargs)
                self.request("PUT", url, request_props)
                return
        # Otherwise, loop and make a bunch of requests.
        log.warning("falling back to individual set for: {} => {}".format(kwargs, devices))
        for device in devices:
            device.set(**kwargs)

    def add_group(self, group: HueLightGroup) -> HueLightGroup:
        self.groups.append(group)

    @classmethod
    def kwargs_to_json(cls, args: dict) -> dict:
        """Parse arguments into a set of request properties for the hue bridge."""
        blob = {}
        if 'on' in args:
            blob['on'] = bool(args['on'])
        if 'color' in args:
            blob.update(HueBridge.color_to_json(args['color']))
        if 'transition_time' in args:
            blob['transitiontime'] = int(args['transition_time'] * 10)
        return blob

    @classmethod
    def color_to_json(cls, color: Color) -> dict:
        """Add the properties from Color to a json object suitable for passing to the hue API."""
        if isinstance(color, BHS):
            return {'bri': color.b, 'hue': color.h, 'sat': color.s}
        elif isinstance(color, Mired):
            return {'ct': color.ct}
        assert isinstance(color, RGB)
        bhs = BHS.from_rgb(color)
        return {'bri': bhs.b, 'hue': bhs.h, 'sat': bhs.s}


class HueLight(Actuator):
    """
    An individually controllable Philips Hue light.
    """
    def __init__(self, name: str, bridge: HueBridge):
        super().__init__(name)
        self.bridge = bridge
        self.hue_light_id = bridge.identify_light(name)

        state = self.bridge.get_state(self.hue_light_id)
        log.info('HueLight(name="{}", id="{}", model="{}", swversion="{}"'.format(name, self.hue_light_id, state.modelid, state.swversion))

    def set(self, **args):
        data = self.kwargs_to_json(args)
        if not data:
            log.warning("skipping HueLight.set because of empty request for args {}".format(args))
            return

        url = "/lights/{}/state".format(self.hue_light_id)
        self.bridge.request("PUT", url, data)

    def kwargs_to_json(self, args: dict) -> dict:
        """Parse arguments into a set of request properties for the hue bridge."""
        blob = {}
        if 'on' in args and self.on != args['on']:
            blob['on'] = bool(args['on'])
        if 'color' in args and self.color != args['color']:
            blob.update(HueBridge.color_to_json(args['color']))
        return blob

    @property
    def modelid(self) -> str:
        return self.bridge.get_state(self.hue_light_id).modelid

    @property
    def swversion(self) -> str:
        return self.bridge.get_state(self.hue_light_id).swversion

    @property
    def on(self) -> bool:
        return self.bridge.get_state(self.hue_light_id).on

    @property
    def color(self) -> Color:
        state = self.bridge.get_state(self.hue_light_id)
        if state.colormode == 'hs':
            return BHS(state.bri, state.hue, state.sat)
        assert state.colormode == 'ct'
        return Mired(state.ct)


