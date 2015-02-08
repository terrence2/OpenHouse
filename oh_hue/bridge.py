# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import json
import logging

from collections import defaultdict, namedtuple
from contextlib import contextmanager
from datetime import datetime, timedelta
from queue import Queue, Empty
from pprint import pprint, pformat

import requests

import shared.util as util

from shared.home import Home


log = logging.getLogger('oh_hue.bridge')


class QueueThread(util.ExitableThread):
    class ExitMarker: pass

    def __init__(self):
        super().__init__()
        self.queue_ = Queue()

    def exit(self):
        self.queue_.put(self.ExitMarker())


# The message type of a light update request.
LightUpdate = namedtuple('LightUpdate', ['request_time', 'light_id', 'json_data'])


class Bridge(QueueThread):
    def __init__(self, name: str, addr: str, username: str, home: Home):
        super().__init__()
        self.home = home
        self.name = name
        self.bridge_query = Home.path_to_query(self.name)

        # Bridge access.
        self.address = addr
        self.username = username

        # Make initial status query.
        res = requests.get(self.url(''))
        self.status_ = res.json()
        log.debug(pformat(self.status_))

        # Derive initial group list.
        self.known_groups_ = {
            frozenset(self.status_['lights'].keys()): '0'
        }
        # TODO: record any other well-known groups.

        # Inject the hue-bridge config under the bridge node.
        query_group = self.home.group()
        query_group.query(self.bridge_query).empty()
        query_group.reflect_as_properties(self.bridge_query, self.status_['config'])
        query_group.run()

        # Keep a list of lights we have queried info for so we can
        # report any that are probably unconfigured.
        self.queried_lights_ = set()

        # TODO: poll for changes to the state set on the bridge and reflect in the HOMe.

        # Keep requests in a window so that we can group requests.
        self.nagle_window_ = []

    def url(self, target: str) -> str:
        """
        Build a url to interact with api |target|.
        """
        return "http://{}/api/{}{}".format(self.address, self.username, target)

    def owns_light_named(self, light_name: str) -> bool:
        for light_id, light_state in self.status_['lights'].items():
            if light_state['name'] == light_name:
                return True
        return False

    def get_id_for_light_named(self, light_name: str) -> bool:
        for light_id, light_state in self.status_['lights'].items():
            if light_state['name'] == light_name:
                self.queried_lights_.add(light_name)
                return light_id
        raise Exception("Attempted to get id for unowned light.")

    def show_unqueried_lights(self):
        for light_id, light_state in self.status_['lights'].items():
            if light_state['name'] not in self.queried_lights_:
                log.error("Found unconfigured light: {}".format(light_state['name']))

    def set_light_state(self, light_id: str, json_data: str):
        log.debug("Light request for {} @ {}".format(light_id, datetime.now()))
        self.queue_.put(LightUpdate(datetime.now(), light_id, json_data))

    # The nagle window is the maximum amount of time between the first message is received and the last message is
    # received that we will wait before sending the message group.
    NAGLE_WINDOW_SIZE = timedelta(seconds=0.050)  # 50ms

    # The nagle window delay is the maximum we will wait for the next message to be received before we decide that the
    # sender has stopped and we should send the existing messages.
    NAGLE_WINDOW_DELAY = timedelta(seconds=0.010)  # 10ms

    # Time to wait when not inside a nagle window.
    NONWINDOW_DELAY = timedelta(seconds=5)  # 5s

    def run(self):
        while True:
            try:
                # Use a longer delay if we're not currently accumulating messages.
                next_delay = self.NAGLE_WINDOW_DELAY if len(self.nagle_window_) > 0 else self.NONWINDOW_DELAY
                entry = self.queue_.get(True, next_delay.total_seconds())

                if isinstance(entry, self.ExitMarker):
                    self.maybe_dispatch_nagle_window()
                    return

                elif isinstance(entry, LightUpdate):
                    self.nagle_window_.append(entry)
                    self.maybe_dispatch_nagle_window()

            except Empty:
                # The nagle delay (or more) has expired, so send any messages.
                self.maybe_dispatch_nagle_window()

    def nagle_window_has_expired(self):
        if not self.nagle_window_:
            return False
        window_size = self.nagle_window_[-1].request_time - self.nagle_window_[0].request_time
        window_delay = datetime.now() - self.nagle_window_[-1].request_time
        log.debug("size:{}; delay:{}".format(window_size, window_delay))
        return window_size > self.NAGLE_WINDOW_SIZE or window_delay > self.NAGLE_WINDOW_DELAY

    @staticmethod
    def assort_groups(updates: [LightUpdate]) -> [(frozenset, str)]:
        """
        Maps the updates list from individual updates to a list with id's grouped by common properties.
            [(_, id, props)] => [(set(id), props)]
        """
        groups = defaultdict(set)
        for _, light_id, json_data in updates:
            groups[json_data].add(light_id)
        out = [(frozenset(v), k) for k, v in groups.items()]
        return sorted(out, key=lambda item: len(item[0]), reverse=True)

    @staticmethod
    def show_response_errors(res: requests.Response) -> bool:
        """
        Returns True if there were no errors in the response.
        """
        errors = [item['error'] for item in res.json() if 'error' in item]
        for error in errors:
            log.error(error)
        return len(errors) == 0

    def update_well_known_group(self, group_id: str, json_data: bytes) -> bool:
        """
        Return True on success.
        """
        url = self.url('/groups/{}/action'.format(group_id))
        res = requests.put(url, data=json_data)
        return self.show_response_errors(res)

    @contextmanager
    def temporary_group(self, group: frozenset):
        create_data = json.dumps({'lights': list(group), 'name': 'temporary'})
        response = requests.post(self.url('/groups'), data=create_data)
        result = self.show_response_errors(response)
        if not result:
            yield None
            return
        group_id = response.json()[0]['success']['id']
        try:
            yield group_id
        finally:
            response = requests.delete(self.url('/groups/{}'.format(group_id)))
            self.show_response_errors(response)

    def maybe_dispatch_nagle_window(self):
        if not self.nagle_window_has_expired():
            return

        group_list = self.assort_groups(self.nagle_window_)
        log.debug("Dispatching {} messages in nagle window in {} groups".format(len(self.nagle_window_), len(group_list)))
        log.debug(pformat(group_list))
        self.nagle_window_ = []

        for group, json_data in group_list:
            # Send a group request for well-known groups.
            log.warning("group {} in known: {} -> len: {}".format(group, group in self.known_groups_, len(group)))
            if group in self.known_groups_:
                if self.update_well_known_group(self.known_groups_[group], json_data):
                    continue

            # Make a temporary group if there is not one we know about.
            elif len(group) > 0:
                with self.temporary_group(group) as group_id:
                    if group_id is not None and self.update_well_known_group(group_id, json_data):
                        continue

            # Otherwise apply the lights manually.
            for light_id in group:
                url = self.url('/lights/{}/state'.format(light_id))
                res = requests.put(url, data=json_data)
                self.show_response_errors(res)
