# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import aiohttp
import logging
import re

from pprint import pformat
from datetime import datetime, timedelta
from urllib.parse import urlparse, urljoin, urlunparse


class WemoDevice:
    def __init__(self, name: str, setup_location: str, callback_addr: (str, int)):
        self.name = name
        self.event_url = urljoin(setup_location, "/upnp/event/basicevent1")
        self.log = logging.getLogger("wemo_device.{}".format(name))

        self.callback_address = "<http://{}:{}>".format(*callback_addr)

        self.last_sid = None

    @staticmethod
    def parse_timeout(header):
        match = re.search(r'Second-(\d+)', header)
        return int(match.group(1))

    @asyncio.coroutine
    def subscribe(self):
        """
        Note that in UPnP, SUBSCRIBE subscribes to /everything/ all the time, so there is no point not just doing it
        automatically.
        """
        assert self.last_sid is None

        response = yield from aiohttp.request('SUBSCRIBE', self.event_url,
                                    headers={'NT': 'upnp:event', 'CALLBACK': self.callback_address})

        # Parse headers.
        timeout = self.parse_timeout(response.headers['TIMEOUT'])
        self.last_sid = response.headers['SID']

        # Setup resubscribe.
        delay = timeout * 3 // 4
        asyncio.async(self.resubscribe_after(delay))

        self.log.debug("subscribed to {} at sid {}, with timeout {} s; resubscribe in {} s.".format(
            self.name, self.last_sid, timeout, delay))
        return self.last_sid

    @asyncio.coroutine
    def resubscribe_after(self, delay: int):
        self.log.debug("will resubscribe in {} seconds".format(delay))
        yield from asyncio.sleep(delay)

        assert self.last_sid is not None
        response = yield from aiohttp.request('SUBSCRIBE', self.event_url,
                                              headers={'SID': self.last_sid})

        # Parse headers.
        timeout = self.parse_timeout(response.headers['TIMEOUT'])
        self.last_sid = response.headers['SID']

        # Setup resubscribe.
        delay = timeout * 3 // 4
        asyncio.async(self.resubscribe_after(delay))

        self.log.debug("resubscribed to {} at sid {}, with timeout {} s; resubscribe in {} s".format(
            self.name, self.last_sid, timeout, delay))


    def on_state_changed(self, state: bool):
        self.log.info("Device {} changed state to {}".format(self.name, state))

