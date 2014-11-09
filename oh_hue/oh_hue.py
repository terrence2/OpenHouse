#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import json
import logging
import os.path

from datetime import datetime
from pprint import pprint, pformat
from threading import RLock

import requests
import tinycss.color3

from prompt_toolkit.contrib.repl import embed

import color
import util

from home import Home, QueryGroup


log = logging.getLogger('oh_hue')


def bool_from_string(value: str) -> bool:
    assert value in ('true', 'false')
    return value == 'true'


def bool_to_string(value: bool) -> str:
    return str(value).lower()


class LightStateCache:
    """
    Keep some properties locally to avoid round-trips.
    """
    DEFAULT_TRANSITION_TIME = 1.0

    def __init__(self, id: str, name: str, path: str, status: dict):
        self.id = id
        self.name = name
        self.path = path
        self.last_change = datetime.now()

        self.colormode = status['state']['colormode']
        self.transition_time_ = self.DEFAULT_TRANSITION_TIME
        self.on = status['state']['on']
        self.bhs = color.BHS(status['state']['bri'], status['state']['hue'], status['state']['sat'])

        if 'ct' in status['state']:
            self.emulates_ct = False
            self.ct = status['state']['ct']
        else:
            self.emulates_ct = True
            self.ct = 510

    @property
    def transition_time(self):
        return int(self.transition_time_)

    @transition_time.setter
    def transition_time(self, value: float):
        self.transition_time_ = value


class Bridge:
    def __init__(self, name: str, addr: str, username: str, home: Home):
        self.home = home
        self.name = name
        self.bridge_query = self.home.path_to_query(self.name)

        # Bridge access.
        self.address = addr
        self.username = username

        # Map from HOMe paths to cached data.
        self.owned_lights_ = {}

        res = requests.get(self.url(''))
        status = res.json()

        # Inject the hue-bridge config under the bridge node.
        group = self.home.group()
        group.query(self.bridge_query).empty()
        group.reflect_as_properties(self.bridge_query, status['config'])

        # Find all configured lights that belong to this bridge.
        lights = self.home.query("[kind='hue']").run()
        for light_path, node in lights.items():
            attrs = node['attrs']
            # FIXME: this should match what's in the config exactly; need to reconfigure locally.
            light_name = 'hue-' + attrs['name']
            light_cache = self.create_cache(light_name, light_path, status['lights'])
            if light_cache:
                self.owned_lights_[light_cache.path] = light_cache
                self.setup_light(group, light_cache)

        group.run()

    @staticmethod
    def create_cache(name: str, path: str, lights: dict) -> LightStateCache:
        for light_id, light_state in lights.items():
            if light_state['name'] == name:
                return LightStateCache(light_id, name, path, light_state)
        return None

    def url(self, target: str) -> str:
        """
        Build a url to interact with api |target|.
        """
        return "http://{}/api/{}{}".format(self.address, self.username, target)

    def setup_light(self, group: QueryGroup, cache: LightStateCache):
        light_query = self.home.path_to_query(cache.path)
        (group.query(light_query).empty()
                                 .attr('on', bool_to_string(cache.on))
                                 .attr('transition_time', str(cache.transition_time)))
        group.query(light_query).append('<div name="bhs" bri="{}" hue="{}" sat="{}"></div>'.format(cache.bhs.b, cache.bhs.h, cache.bhs.s))
        group.query(light_query).append('<div name="ct" ct="{}"></div>'.format(cache.ct))

        self.home.subscribe(cache.path, self.on_light_state_change)
        self.home.subscribe(cache.path + '/bhs', self.on_light_bhs_change)
        self.home.subscribe(cache.path + '/ct', self.on_light_ct_change)

    @staticmethod
    def parse_css(style: str) -> dict:
        out = {}
        parts = style.strip(';').split(';')
        for part in parts:
            key, _, value = part.strip().partition(':')
            out[key.strip()] = value.strip()
        return out

    def on_light_state_change(self, target: str, node: dict):
        attributes = node['attrs']
        cache = self.owned_lights_[target]
        log.info("Light state change: {} => {}".format(cache.name, attributes))
        device_is_on = cache.on

        if 'on' in attributes:
            cache.on = bool_from_string(attributes['on'])

        if 'transition_time' in attributes:
            cache.transition_time = int(float(attributes['transition_time']) * 10)

        if 'style' in attributes:
            css = self.parse_css(attributes['style'])
            if 'color' in css:
                rgba = tinycss.color3.parse_color_string(css['color'])
                cache.colormode = 'rgb'
                cache.bhs = color.BHS.from_css(rgba)

        if 'bri' in attributes or 'hue' in attributes or 'sat' in attributes:
            cache.bhs = color.BHS(
                attributes.get('bri', cache.bhs.b),
                attributes.get('hue', cache.bhs.h),
                attributes.get('sat', cache.bhs.s)
            )

        parameters = {
            'on': cache.on,
            'transitiontime': cache.transition_time,
        }
        if device_is_on or cache.on:
            parameters.update({
                'bri': cache.bhs.b,
                'hue': cache.bhs.h,
                'sat': cache.bhs.s
            })
        url = self.url('/lights/{}/state'.format(cache.id))
        data = json.dumps(parameters).encode('UTF-8')
        log.warning("Sending: {} <= {}".format(url, data))
        res = requests.put(url, data=data)
        for item in res.json():
            if 'error' in item:
                log.error(item['error'])

    def on_light_bhs_change(self, target: str, node: dict):
        parent = os.path.dirname(target)
        self.on_light_state_change(parent, node)

    def on_light_ct_change(self, target: str, node: dict):
        pass


def find_bridges(home: Home) -> [Bridge]:
    res = home.query("[kind='hue-bridge']").run()
    return [Bridge(name, node['attrs']['ipv4'], node['attrs']['username'], home) for name, node in res.items()]


def main():
    util.enable_logging('events.log', 'DEBUG')

    with util.run_thread(Home((3, 0), RLock())) as home:
        bridges = find_bridges(home)
        embed(globals(), locals(), vi_mode=True)


if __name__ == '__main__':
    main()
