# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import aiohttp
import asyncio
import itertools
import json
import logging
from collections import defaultdict, namedtuple
from datetime import datetime, timedelta
from oh_shared.home import Home

log = logging.getLogger('oh_hue.bridge')


# The message type of a light update request.
LightUpdate = namedtuple('LightUpdate', ['request_time', 'light_id', 'json_data'])


class Bridge:
    def __init__(self, path: str, addr: str, username: str, home: Home):
        super().__init__()
        self.home = home
        self.path = path
        self.bridge_query = Home.path_to_query(self.path)

        # Bridge access.
        self.address = addr
        self.username = username

        # Keep a list of lights we have queried info for so we can
        # report any that are probably unconfigured.
        self.queried_lights_ = set()

        # TODO: poll for changes to the state set on the bridge and reflect in the HOMe.

        # Keep requests in a window so that we can group requests.
        self.nagle_window_ = []

        # Set to true while the watching task is running.
        self.watching = False

        # Set async after creation:
        self.status_ = {}
        self.known_groups_ = {}
        self.temp_group_id_ = itertools.count()

    @classmethod
    @asyncio.coroutine
    def create(cls, path: str, addr: str, username: str, home: Home) -> 'Bridge':
        bridge = cls(path, addr, username, home)

        # Make initial status query.
        res = yield from aiohttp.request('GET', bridge.url(''))
        bridge.status_ = yield from res.json()
        for i, (light_id, props) in enumerate(bridge.status_['lights'].items()):
            log.debug("light#{:<2} {:>2} : {:20} : {} : {}".format(i, light_id, props['name'], props['modelid'],
                                                                   props['swversion'], props['uniqueid']))
        #log.debug(pformat(bridge.status_))

        # Derive initial group list.
        bridge.known_groups_ = {
            frozenset(bridge.status_['lights'].keys()): '0'
        }
        # TODO: record any other well-known groups.

        return bridge

    def url(self, target: str) -> str:
        """
        Build a url to interact with api |target|.
        """
        return "http://{}/api/{}{}".format(self.address, self.username, target)

    def owns_light_named(self, light_name: str) -> bool:
        """
        Return true if the given light is controlled by this bridge. In theory, two bridges could have
        a light with the same name: we simply assume this does not happen.
        """
        for light_id, light_state in self.status_['lights'].items():
            if light_state['name'] == light_name:
                return True
        return False

    def get_id_for_light_named(self, light_name: str) -> str:
        """
        Map a light name (as configured in openhouse and the hue gui) to a light id used for controlling the light.
        """
        for light_id, light_state in self.status_['lights'].items():
            if light_state['name'] == light_name:
                self.queried_lights_.add(light_name)
                return light_id
        raise Exception("Attempted to get id for unowned light.")

    def show_unqueried_lights(self):
        """
        Log any lights that have not had their name->id mapping requested.
        """
        for light_id, light_state in self.status_['lights'].items():
            if light_state['name'] not in self.queried_lights_:
                log.error("Found unconfigured light: {}".format(light_state['name']))

    # The nagle window is the maximum amount of time between the first message is received and the last message is
    # received that we will wait before sending the message group.
    NAGLE_WINDOW_SIZE = timedelta(seconds=0.050)  # 50ms

    # The nagle window delay is the maximum we will wait for the next message to be received before we decide that the
    # sender has stopped and we should send the existing messages.
    NAGLE_WINDOW_DELAY = timedelta(seconds=0.010)  # 10ms

    # Time to wait when not inside a nagle window.
    NONWINDOW_DELAY = timedelta(seconds=5)  # 5s

    @asyncio.coroutine
    def set_light_state(self, light_id: str, json_data: str):
        log.debug("Light request for {} @ {}".format(light_id, datetime.now()))
        self.nagle_window_.append(LightUpdate(datetime.now(), light_id, json_data))
        if self.watching:
            return
        self.watching = True
        yield from asyncio.async(self.watch_window())

    @asyncio.coroutine
    def watch_window(self):
        while self.nagle_window_:
            yield from self.maybe_dispatch_nagle_window()
            yield from asyncio.sleep(0.050)
        self.watching = False

    def nagle_window_has_expired(self):
        if not self.nagle_window_:
            return False
        window_size = self.nagle_window_[-1].request_time - self.nagle_window_[0].request_time
        window_delay = datetime.now() - self.nagle_window_[-1].request_time
        log.debug("size:{}; delay:{}".format(window_size, window_delay))
        return window_size > self.NAGLE_WINDOW_SIZE or window_delay > self.NAGLE_WINDOW_DELAY

    @asyncio.coroutine
    def maybe_dispatch_nagle_window(self):
        """
        Dispatch the nagle window if the window has expired. Return True if we sent the window.
        """
        if not self.nagle_window_has_expired():
            return

        group_list = self.assort_groups(self.nagle_window_)
        self.nagle_window_ = []

        log.debug("Dispatching {} messages in nagle window in {} groups".format(len(self.nagle_window_), len(group_list)))
        for group, json_data in group_list:
            log.debug("Sending {} to:".format(json_data))
            light_names = sorted([(self.status_['lights'][light_id]['name'], light_id) for light_id in group])
            for name, lid in light_names:
                log.debug("\t{:20} @ {}".format(name, lid))

        for item in group_list:
            yield from self.update_group(*item)

    @staticmethod
    def assort_groups(updates: [LightUpdate]) -> [(frozenset, bytes)]:
        """
        Maps the updates list from individual updates to a list with id's grouped by common properties.
            [(_, id, props)] => [(set(id), props)]
        """
        groups = defaultdict(set)
        for _, light_id, json_data in updates:
            groups[json_data].add(light_id)
        out = [(frozenset(v), k) for k, v in groups.items()]
        return sorted(out, key=lambda item: len(item[0]), reverse=True)

    @asyncio.coroutine
    def update_group(self, group: frozenset, json_data: bytes):
        """
        Set a group of lights to state given by json_data.
        """
        # Send a group request for well-known groups.
        if group in self.known_groups_:
            yield from self.update_well_known_group(self.known_groups_[group], json_data)
            yield from asyncio.sleep(0.100)

        # Make a temporary group if there is not one we know about.
        # It takes a request each to create, set, and delete a group,
        # so only both if we will save at least one request.
        elif len(group) > 3:
            yield from self.update_unknown_group(group, json_data)
            yield from asyncio.sleep(0.300)

        # Otherwise apply the lights manually.
        else:
            for light_id in group:
                yield from self.update_single_light(light_id, json_data)
                yield from asyncio.sleep(0.50)

    @asyncio.coroutine
    def update_well_known_group(self, group_id: str, json_data: bytes) -> bool:
        """
        Set the state of a pre-configured group of lights. Return True on success.
        """
        url = self.url('/groups/{}/action'.format(group_id))
        log.debug("PUT {} <- {}".format(url, json_data))
        response = yield from aiohttp.request("PUT", url, data=json_data)
        content = yield from response.json()
        self.show_response_errors(content)

    @asyncio.coroutine
    def update_unknown_group(self, group: frozenset, json_data: bytes):
        """
        Set the state of an unconfigured group of lights. Return True on success.
        """
        create_data = json.dumps({'lights': list(group), 'name': 'temp' + str(next(self.temp_group_id_))})
        log.debug("POST {} <- {}".format(self.url('/groups'), create_data))
        response = yield from aiohttp.request('POST', self.url('/groups'), data=create_data)
        content = yield from response.json()
        result = self.show_response_errors(content)
        if not result:
            return
        group_id = content[0]['success']['id']
        try:
            yield from self.update_well_known_group(group_id, json_data)
        finally:
            log.debug("DELETE {}".format(self.url('/groups/{}'.format(group_id))))
            response = yield from aiohttp.request('DELETE', self.url('/groups/{}'.format(group_id)))
            content = yield from response.json()
            self.show_response_errors(content)

    @asyncio.coroutine
    def update_single_light(self, light_id: str, json_data: bytes) -> bool:
        """
        Set the state of a single light. Return True on success.
        """
        url = self.url('/lights/{}/state'.format(light_id))
        log.debug("PUT {} <- {}".format(url, json_data))
        response = yield from aiohttp.request("PUT", url, data=json_data)
        content = yield from response.json()
        self.show_response_errors(content)

    @staticmethod
    def show_response_errors(response: object) -> bool:
        """
        Returns True if there were no errors in the response.
        """
        errors = [item['error'] for item in response if 'error' in item]
        for error in errors:
            log.error(error)
        return len(errors) == 0

