# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from oh_shared.db.tree import Tree
from oh_shared.color import parse_css_color, BHS, RGB, Mired
import aiohttp
import json
import logging
from pathlib import PurePosixPath as Path
from datetime import datetime, timedelta

log = logging.getLogger('oh_hue.bridge')


class HueException(Exception):
    pass


class Bridge:
    @classmethod
    async def create(cls, tree: Tree) -> 'Bridge':
        self = cls()
        self.address = await tree.get_file("/global/hue-bridge/address")
        self.username = await tree.get_file("/global/hue-bridge/username")
        self.transition_time = int(await tree.get_file("/global/hue-bridge/transition_time"))

        # Watch for changes to the transition time.
        def on_transition_time_updated(_0, _1, context: str):
            try:
                self.transition_time = int(context)
            except ValueError:
                pass
        await tree.subscribe("/meta/hardware/hue-bridge/transition_time", on_transition_time_updated)

        # Make initial status query.
        res = await aiohttp.request('GET', self.url(''))
        status = await res.json()
        config = status['config']
        interesting = ('name', 'modelid', 'bridgeid', 'apiversion', 'swversion',
                       'UTC', 'localtime', 'timezone',
                       'ipaddress', 'mac', 'gateway', 'netmask', 'zigbeechannel')
        log.info("Hue Bridge Configuration:")
        for prop in interesting:
            log.info("{}: {}".format(prop, config[prop]))

        # Query the database for all configured light names.
        matching_lights = await tree.get_matching_files("/room/*/hue-*/*/color")
        light_names = {Path(p).parent.name for p in matching_lights.keys()}

        # Print light configuration and build light name map.
        self.lights_by_name = {}
        for i, (light_id, props) in enumerate(status['lights'].items()):
            log.info("light#{:<2} {:>2} : {:20} : {} : {}".format(i, light_id, props['name'], props['modelid'],
                                                                  props['swversion'], props['uniqueid']))
            assert props['name'] not in self.lights_by_name, "Duplicate name detected!"
            if props['name'] not in light_names:
                log.warning("Found unconfigured light names {}. You should add it to your"
                            "configuration tree.")
            self.lights_by_name[props['name']] = light_id
        for name in light_names:
            if name not in self.lights_by_name:
                log.warning("The configured light named {} was not found on the bridge;"
                            "do you need to associate it with your hub still?")

        # Remove any pre-existing groups.
        for i, (group_id, props) in enumerate(status['groups'].items()):
            log.warning("removing pre-existing group {}".format(group_id))
            response = await aiohttp.request('DELETE', self.url('/groups/{}'.format(group_id)))
            content = await response.json()
            self.show_response_errors(content)

        # The default group '0' contains all lights.
        self.groups = {
            tuple(sorted(list(light_names))): '0'
        }

        # Store the prior update so that if a duplicate comes in (e.g. from someone
        # hammering on the button) we can just ignore it.
        self.last_update = (datetime.now(), ([], ''))

        return self

    @staticmethod
    def show_response_errors(response: object) -> bool:
        """
        Returns True if there were no errors in the response.
        """
        errors = [item['error'] for item in response if 'error' in item]
        for error in errors:
            log.error(error)
        if errors:
            raise HueException(errors)

    @staticmethod
    def light_state_from_color(raw_color: str) -> str:
        on_state = raw_color != 'none'
        props = {'on': on_state}
        if on_state:
            color = parse_css_color(raw_color)
            props.update({'transitiontime': 10})
            if isinstance(color, BHS):
                props.update({
                    'bri': color.b,
                    'hue': color.h,
                    'sat': color.s,
                })
            else:
                assert isinstance(color, Mired)
                props.update({'ct': color.ct})
        return json.dumps(props).encode('UTF-8')

    async def set_lights_to_color(self, names: [str], color: str):
        log.info("Setting lights {} to color: {}".format(names, color))
        assert len(names) > 0, "no names in set_lights_to_color"

        key = tuple(sorted(names))
        if datetime.now() - self.last_update[0] > timedelta(seconds=0.5) and \
           self.last_update[1] == (key, color):
            log.warning("ignoring duplicate update call")
            return

        # If there is only a single group to update, just send it alone.
        if len(names) == 1:
            return await self.update_single_light(names[0], color)

        if key not in self.groups:
            await self.create_new_grouping(key)
        await self.update_group(key, color)

        self.last_update = (datetime.now(), (key, color))

    async def create_new_grouping(self, key: (str,)):
        log.info("Creating new group for {}".format(key))
        ids = [self.lights_by_name[name] for name in key]
        create_data = json.dumps({'lights': ids})
        log.debug("POST {} <- {}".format(self.url('/groups'), create_data))
        response = await aiohttp.request('POST', self.url('/groups'), data=create_data)
        content = await response.json()
        self.show_response_errors(content)
        log.warning("result is: {}".format(content))
        group_id = content[0]['success']['id']
        self.groups[key] = group_id

    async def update_group(self, key: (str,), color: str):
        group_id = self.groups[key]
        log.info("Setting group {} to {}".format(group_id, color))
        url = self.url('/groups/{}/action'.format(group_id))
        json_data = self.light_state_from_color(color)
        log.debug("PUT {} <- {}".format(url, json_data))
        response = await aiohttp.request("PUT", url, data=json_data)
        content = await response.json()
        self.show_response_errors(content)

    async def update_single_light(self, name: str, color: str):
        light_id = self.lights_by_name[name]
        url = self.url('/lights/{}/state'.format(light_id))
        json_data = self.light_state_from_color(color)
        log.debug("PUT {} <- {}".format(url, json_data))
        response = await aiohttp.request("PUT", url, data=json_data)
        content = await response.json()
        self.show_response_errors(content)

    def url(self, target: str) -> str:
        """
        Build a url to interact with api |target|.
        """
        return "http://{}/api/{}{}".format(self.address, self.username, target)

