#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import asyncio
import logging
import re
import shared.util as util

from aiohttp import web
from pprint import pformat
from shared.home import Home
from threading import RLock

from wemo_discoverer import discover_local_wemos, WemoDeviceInfo
from wemo_device import WemoDevice

log = logging.getLogger('oh_wemo')


def main():
    parser = argparse.ArgumentParser(description='Bridge between OpenHouse and local WeMo devices.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    with util.run_thread(Home((3, 0), RLock())) as home:
        asyncio.get_event_loop().run_until_complete(manage_devices(home))
        try:
            asyncio.get_event_loop().run_forever()
        except KeyboardInterrupt:
            return 0



import aiohttp
import aiohttp.server


class HttpRequestHandler(aiohttp.server.ServerHttpProtocol):
    def __init__(self, devices_by_sid: {str: WemoDevice}, home: Home, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.devices_by_sid = devices_by_sid
        self.home = home

    @asyncio.coroutine
    def handle_state_change(self, device: WemoDevice, state: bool):
        log.info("Device {} changed state to {}".format(device.name, state))
        self.home.query("[name='{}']".format(device.name)).attr('state', state).run()

    @asyncio.coroutine
    def handle_request(self, message, payload):

        # Ensure this is actually a notification for a WeMo.
        if (message.method != 'NOTIFY' or
                'NT' not in message.headers or
                message.headers['NT'] != 'upnp:event' or
                'NTS' not in message.headers or
                message.headers['NTS'] != 'upnp:propchange' or
                'SID' not in message.headers):
            log.warning("got unexpected message from {}".format('eh?'))
            return

        # Check that we have this sid.
        sid = message.headers['SID']
        device = self.devices_by_sid.get(sid, None)
        if device is None:
            peer_name = self.transport.get_extra_info('peername')
            log.warning("got message from {} with unknown sid: {}".format(peer_name, sid))
            return

        # Read the message payload out.
        raw_data = yield from payload.read()

        # Respond with a minimal response.
        response = aiohttp.Response(
            self.writer, 200, http_version=message.version
        )
        response.add_header('Content-Type', 'text/html')
        response.add_header('Content-Length', '0')
        response.send_headers()
        yield from response.write_eof()

        # "Parse" the message.
        content = raw_data.decode('ASCII')
        matches = re.search(r'<BinaryState>(\d)</BinaryState>', content)
        if not matches:
            # Subscribe subscribes to /all/ states all the time, this may just not be an event message.
            return
        state = bool(int(matches.group(1)))

        # Emit the state change.
        asyncio.async(self.handle_state_change(self.devices_by_sid[sid], state))

        #log.warning("GOT REQUEST: {}, {}".format(message, dir(payload)))
        #log.warning("DATA: {} => {}".format(raw_data, state))

@asyncio.coroutine
def manage_devices(home: Home):
    # Get a list of all devices.
    switches = {}
    nodes = home.query("wemo-switch").run()
    for path, node in nodes.items():
        name = node['attrs']['name']
        switches[name] = None
    device_names = switches.keys()

    # The global devices list.
    devices_by_name = {}
    devices_by_sid = {}

    # Start the reply server, sending events to the wemo devices in |devices|.
    srv = yield from asyncio.get_event_loop().create_server(
        lambda: HttpRequestHandler(devices_by_sid, home, debug=True, keep_alive=75),
        util.get_own_internal_ip_slow())
    callback_addr = srv.sockets[0].getsockname()
    log.info('serving on {}'.format(callback_addr))

    # Start looking for wemos, passing them our server's address.
    @asyncio.coroutine
    def found_new_device(info: WemoDeviceInfo):
        if info.name in device_names:
            device = WemoDevice(info.name, info.location, callback_addr)
            sid = yield from device.subscribe()
            devices_by_name[info.name] = device
            devices_by_sid[sid] = device
    yield from discover_local_wemos(found_new_device)

if __name__ == '__main__':
    main()

