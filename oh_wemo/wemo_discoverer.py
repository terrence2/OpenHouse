#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import aiohttp
import asyncio
import logging
import re

from collections import namedtuple

import shared.util as util

log = logging.getLogger("oh_wemo.discoverer")


class FormatException(Exception):
    pass


WemoDeviceInfo = namedtuple("WemoDeviceInfo", ['name', 'location'])


class WemoDiscoveryProtocol:
    MulticastAddress = ('239.255.255.250', 1900)
    UPnPSearch = ('\r\n'.join(("M-SEARCH * HTTP/1.1",
                               "HOST:{}:{}",
                               "ST:upnp:rootdevice",
                               "MX:2",
                               'MAN:"ssdp:discover"',
                               "", "")).format(*MulticastAddress)).encode("UTF-8")

    def __init__(self, connected: asyncio.Future, wemo_coro: callable):
        """
        Takes a mapping of expected wemo names to futures. Each future will be triggered with
        a WemoDeviceInfo as the device is found on the network. Also takes a coroutine to trigger
        when a wemo not in that set is found.
        """
        self.wemo_coro_ = wemo_coro
        self.seen_devices_ = set()
        self.connected_ = connected

    def connection_made(self, transport):
        self.connected_.set_result(transport)

    def datagram_received(self, data, client_address):
        if client_address[0] in self.seen_devices_:
            log.debug("skipping already seen device at {}".format(client_address[0]))
            return
        self.seen_devices_.add(client_address[0])
        asyncio.async(self.handle_maybe_wemo(data, client_address))

    def send_search_request(self, transport):
        transport.sendto(self.UPnPSearch, self.MulticastAddress)

    @asyncio.coroutine
    def handle_maybe_wemo(self, data: bytes, client_address: (str, int)):
        try:
            headers = self.parse_http_to_headers(data)
        except FormatException as ex:
            log.exception(ex)
            return

        if headers.get('x-user-agent', None) != 'redsonic':
            return log.debug("Found non-wemo device at: {}".format(client_address))

        log.debug("Found WeMo at {}".format(client_address))

        # Query the location to get the device's name.
        response = yield from aiohttp.request('GET', headers['location'])
        assert response.status == 200
        data = yield from response.text()
        matches = re.search(r'<friendlyName>(.*?)</friendlyName>', data)
        if not matches:
            return log.warning("Found WeMo-like device without a <friendlyName> at {}".format(client_address))
        device_name = matches.group(1)
        info = WemoDeviceInfo(device_name, headers['location'])

        # Call the callback outside the event loop.
        yield from self.wemo_coro_(info)

    @staticmethod
    def parse_http_to_headers(raw_request: bytes) -> {}:
        """
        Parse some request text and pull out the headers.
        """
        request = raw_request.decode('UTF-8')
        lines = [line.strip() for line in request.split('\n')]
        status_line, headers = lines[0], lines[1:]

        if '200 OK' not in status_line:
            raise Exception("Expected 200 OK, got: " + status_line)

        out = {}
        for line in headers:
            name, _, value = line.partition(':')
            if name:
                out[name.strip().lower()] = value.strip()

        return out


@asyncio.coroutine
def discover_local_wemos(wemo_coro):
    connected = asyncio.Future()
    transport, protocol = yield from asyncio.get_event_loop() \
        .create_datagram_endpoint(lambda: WemoDiscoveryProtocol(connected, wemo_coro),
                                  local_addr=(util.get_own_internal_ip_slow(), 54321))
    yield from connected
    assert connected.result() == transport

    protocol.send_search_request(transport)
    yield from asyncio.sleep(5)
    while True:
        protocol.send_search_request(transport)
        yield from asyncio.sleep(20 * 60)


if __name__ == '__main__':
    util.enable_logging('output.log', 'INFO')
    @asyncio.coroutine
    def report(info):
        log.info("{} @ {}".format(info.name, info.location))
    asyncio.get_event_loop().run_until_complete(discover_local_wemos(report))
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
