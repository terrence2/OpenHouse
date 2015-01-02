# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import requests
from home import Home
import json
import logging
from pprint import pprint


log = logging.getLogger('oh_hue.bridge')


class Bridge:
    def __init__(self, name: str, addr: str, username: str, home: Home):
        self.home = home
        self.name = name
        self.bridge_query = self.home.path_to_query(self.name)

        # Bridge access.
        self.address = addr
        self.username = username

        # Make initial status query.
        res = requests.get(self.url(''))
        self.status_ = res.json()
        pprint(self.status_)

        # Inject the hue-bridge config under the bridge node.
        group = self.home.group()
        group.query(self.bridge_query).empty()
        group.reflect_as_properties(self.bridge_query, self.status_['config'])
        group.run()

        # TODO: poll for changes to the state set on the bridge and reflect in the HOMe.

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
                return light_id
        raise Exception("Attempted to get id for unowned light.")

    def set_light_state(self, light_id: str, properties: dict):
        url = self.url('/lights/{}/state'.format(light_id))
        data = json.dumps(properties).encode('UTF-8')
        log.debug("Sending: {} <= {}".format(url, data))
        res = requests.put(url, data=data)
        for item in res.json():
            if 'error' in item:
                log.error(item['error'])
