# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import aiohttp
import logging
import re

from urllib.parse import urljoin


class WemoDevice:
    def __init__(self, name: str, setup_location: str, callback_address: (str, int)):
        self.name = name
        self.event_url = urljoin(setup_location, "/upnp/event/basicevent1")
        self.log = logging.getLogger("wemo_device.{}".format(name))
        self.callback_address = "<http://{}:{}>".format(*callback_address)

    @staticmethod
    def parse_response_headers(headers: {str: str}) -> (str, int, int):
        """
        Returns: SID, timeout, delay
        """
        sid_header = headers['SID']
        timeout_header = headers['TIMEOUT']

        match = re.search(r'Second-(\d+)', timeout_header)
        timeout = int(match.group(1))

        return sid_header, timeout, (timeout * 3 // 4)

    @asyncio.coroutine
    def follow_device(self, device_map):
        """
        Note that in UPnP, SUBSCRIBE subscribes to /everything/ all the time, so there is no point not just doing it
        automatically.
        """
        headers = {'NT': 'upnp:event', 'CALLBACK': self.callback_address}
        response = yield from aiohttp.request('SUBSCRIBE', self.event_url, headers=headers)
        try:
            sid, timeout, delay = self.parse_response_headers(response.headers)
        except KeyError as ex:
            self.log.exception(ex)
            self.log.error("Failed to subscribe to device: {}".format(self.name))
            return
        assert sid not in device_map
        device_map[sid] = self
        self.log.debug("subscribed to {} at sid {}, with timeout {} s; resubscribe in {} s.".format(
                       self.name, sid, timeout, delay))

        while True:
            yield from asyncio.sleep(delay)
            headers = {'SID': sid}
            response = yield from aiohttp.request('SUBSCRIBE', self.event_url, headers=headers)
            try:
                next_sid, timeout, delay = self.parse_response_headers(response.headers)
            except KeyError as ex:
                self.log.exception(ex)
                self.log.error("Failed to resubscribe to device: {}".format(self.name))
                del device_map[sid]
                return
            assert sid == next_sid
            self.log.debug("subscribed to {} at sid {}, with timeout {} s; resubscribe in {} s.".format(
                self.name, sid, timeout, delay))
