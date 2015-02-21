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
    def _subscribe(self, device_map: {str: 'WemoDevice'}) -> (str, int, int):
        # Retry until we find the device, waiting between retries.
        backoff = 60  # 1 minute
        max_backoff = 4 * 60 * 60  # 4 hours
        while True:
            headers = {'NT': 'upnp:event', 'CALLBACK': self.callback_address}
            response = yield from aiohttp.request('SUBSCRIBE', self.event_url, headers=headers)
            try:
                sid, timeout, delay = self.parse_response_headers(response.headers)
            except KeyError as ex:
                self.log.exception(ex)
                self.log.error("Failed to subscribe to device: {}".format(self.name))
                yield from asyncio.sleep(backoff)
                backoff = min(max_backoff, backoff * 2)
                continue
            assert sid not in device_map
            device_map[sid] = self
            self.log.debug("subscribed to {} at sid {}, with timeout {} s; resubscribe in {} s.".format(
                self.name, sid, timeout, delay))
            return sid, timeout, delay

    @asyncio.coroutine
    def _resubscribe(self, sid: str, device_map: {str: 'WemoDevice'}) -> (str, int, int):
        headers = {'SID': sid}
        response = yield from aiohttp.request('SUBSCRIBE', self.event_url, headers=headers)
        try:
            next_sid, timeout, delay = self.parse_response_headers(response.headers)
        except KeyError as ex:
            self.log.exception(ex)
            self.log.error("Failed to resubscribe to device: {}".format(self.name))
            del device_map[sid]
            return self._subscribe(device_map)
        assert sid == next_sid
        self.log.debug("re-subscribed to {} at sid {}, with timeout {} s; resubscribe in {} s.".format(
            self.name, sid, timeout, delay))
        return sid, timeout, delay

    @asyncio.coroutine
    def follow_device(self, device_map: {str: 'WemoDevice'}):
        """
        Note that in UPnP, SUBSCRIBE subscribes to /everything/ all the time, so there is no point not just doing it
        automatically.
        """
        sid, timeout, delay = yield from self._subscribe(device_map)
        while True:
            yield from asyncio.sleep(delay)
            sid, timeout, delay = yield from self._resubscribe(sid, device_map)
