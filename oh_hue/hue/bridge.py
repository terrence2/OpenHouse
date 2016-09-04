# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from oh_shared.db.tree import Tree
from oh_shared.color import parse_css_color, BHS, RGB, Mired
import asyncio
import aiohttp
import json
import logging
from collections import namedtuple
from pathlib import PurePosixPath as Path
from datetime import datetime, timedelta

log = logging.getLogger('oh_hue.bridge')


class HueException(Exception):
    pass


class WatchedProperty:
    def __init__(self):
        self.value_ = None
        self.subscription_ = None

    @classmethod
    async def create(cls, tree: Tree, prop: str) -> 'WatchedProperty':
        self = cls()
        self.value_ = await tree.get_file(prop)

        def update_value(_0, _1, context: str):
            self.value_ = context
        self.subscription_ = await tree.subscribe(prop, update_value)

        return self

    @property
    def value(self):
        return self.value_


class Bridge:
    # The type of events stored in the queue.
    Event = namedtuple('Event', ('key', 'color'))

    def __init__(self):
        self.address = None
        self.username = None
        self.lights_by_name = {}
        self.transition_time_ = None
        self.groups = {}
        self.queue = asyncio.Queue()
        self.task = None

    @classmethod
    async def create(cls, tree: Tree) -> 'Bridge':
        self = cls()
        self.address = await tree.get_file("/global/hue-bridge/address")
        self.username = await tree.get_file("/global/hue-bridge/username")

        tt_path = "/global/hue-bridge/transition_time"
        self.transition_time_ = await WatchedProperty.create(tree, tt_path)

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

        # All lights in a single change are delivered at once, so deciding
        # what to change is easy. Unfortunately, multiple change requests can come
        # in at once, flooding the Hue bridge, duplicating groups, etc. Thus
        # we serialize. We could do this with a simple lock, but since there
        # is no back-pressure, we may easily end up with hours worth of
        # un-interruptable light changes. Instead, we use a sorted queue and
        # only take the last change to a set of lights at once.
        self.task = asyncio.Task(self.process_events())

        return self

    @property
    def transition_time(self):
        return float(self.transition_time_.value)

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

    def light_state_from_color(self, raw_color: str) -> dict:
        on_state = raw_color != 'none'
        props = {'on': on_state}
        if on_state:
            color = parse_css_color(raw_color)
            props['transitiontime'] = int(self.transition_time * 10)
            if isinstance(color, BHS):
                props.update({
                    'bri': color.b,
                    'hue': color.h,
                    'sat': color.s,
                })
            else:
                assert isinstance(color, Mired)
                props.update({'ct': color.ct})
        return props

    def url(self, target: str) -> str:
        """
        Build a url to interact with api |target|.
        """
        return "http://{}/api/{}{}".format(self.address, self.username, target)

    async def set_lights_to_color(self, names: [str], color: str):
        assert len(names) > 0, "no names in set_lights_to_color"
        key = tuple(sorted(names))
        await self.queue.put(self.Event(key, color))

    async def process_events(self):
        events = {}
        while True:
            # Wait for an event to arrive.
            event = await self.queue.get()
            events[event.key] = event.color

            # Apply all events that we know about.
            while len(events):
                # Extract any new events that have arrived while we applied the
                # last event.
                while not self.queue.empty():
                    event = self.queue.get_nowait()
                    events[event.key] = event.color

                (key, color) = events.popitem()
                await self.apply_color_to_lights(key, color)

    async def apply_color_to_lights(self, key: (str,), color: str):
        log.info("Setting lights {} to color: {}".format(key, color))

        # Do light color processing up front to simplify our work.
        light_state = self.light_state_from_color(color)

        # If there are only a few lights to update, it is faster to just do them
        # one by one without maintaining a group.
        # FIXME: we do not maintain the 'on' state locally, so it's not all
        # FIXME: that much faster. In theory we could do up to ~10 if we tracked it.
        if len(key) <= 1:
            for name in key:
                return await self.update_single_light(name, light_state)

        # If we have not yet seen this organization of lights, create a group for them.
        if key not in self.groups:
            await self.create_new_grouping(key)

        await self.update_group(key, light_state)

    async def create_new_grouping(self, key: (str,)):
        log.info("Creating new group for {}".format(key))

        # The hub can currently only handle 64 groups at once. More than enough
        # for common residential settings. May need review for deployment in offices
        # although it's possible that one hue bridge cannot reach more than 64 rooms
        # in any case.
        assert len(self.groups) < 64, "too many groups created"

        # Create the group-creation blob.
        ids = [self.lights_by_name[name] for name in key]
        create_data = json.dumps({'lights': ids})

        # Send the request to the hub.
        # FIXME: we can probably recover on failure.
        log.debug("POST {} <- {}".format(self.url('/groups'), create_data))
        response = await aiohttp.request('POST', self.url('/groups'), data=create_data)
        content = await response.json()
        self.show_response_errors(content)

        # Pull out the result.
        group_id = content[0]['success']['id']
        self.groups[key] = group_id

        # In theory we should wait here for a bit to let the bridge process.
        # In practice, we are waiting elsewhere to guarantee that the bridge's
        # queue is always low. This lets us immediately send the followup
        # /group/action request without too much fear that it will not be able
        # to enqueue. This theoretically minimizes our delay, even if we have
        # not seen a light configuration before.

    async def update_group(self, key: (str,), light_state: dict):
        # Lookup the group id and build the url.
        group_id = self.groups[key]
        log.info("Setting group {} to {}".format(group_id, light_state))
        url = self.url('/groups/{}/action'.format(group_id))

        # FIXME: track 'on' state and strip it if we can.

        # Send the light color.
        json_data = json.dumps(light_state).encode("UTF-8")
        log.debug("PUT {} <- {}".format(url, json_data))
        response = await aiohttp.request("PUT", url, data=json_data)
        content = await response.json()
        self.show_response_errors(content)

        # Wait to give the bridge some time to process. The docs say to wait
        # for a full second, but we can generally get away with much less if
        # we handle errors somewhat gracefully.
        await asyncio.sleep(0.4)

    async def update_single_light(self, name: str, light_state: dict):
        # Get the light id and build the url.
        light_id = self.lights_by_name[name]
        url = self.url('/lights/{}/state'.format(light_id))

        # Build the json blob.
        json_data = json.dumps(light_state).encode("UTF-8")

        # FIXME: track 'on' state and strip it if we can.

        # Send the request to the light.
        log.debug("PUT {} <- {}".format(url, json_data))
        response = await aiohttp.request("PUT", url, data=json_data)
        content = await response.json()
        self.show_response_errors(content)

        # If we did not toggle the light state, wait 100ms, otherwise double.
        min_wait = 0.2 if 'on' in json_data else 0.1
        wait_timeout = max(self.transition_time, min_wait)
        await asyncio.sleep(wait_timeout)

