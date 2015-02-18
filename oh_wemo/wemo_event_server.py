# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import aiohttp
import aiohttp.server
import logging
import re

from shared.aiohome import Home
from shared.util import get_own_internal_ip_slow
from wemo_device import WemoDevice

log = logging.getLogger('oh_wemo.event_server')


class WemoEventHandler(aiohttp.server.ServerHttpProtocol):
    def __init__(self, devices_by_sid: {str, WemoDevice}, home: Home, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.devices_by_sid = devices_by_sid
        self.home = home

    @asyncio.coroutine
    def handle_state_change(self, device: WemoDevice, state: bool):
        log.info("Device {} changed state to {}".format(device.name, state))
        yield from self.home("[name='{}']".format(device.name)).attr('state', state).run()

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


@asyncio.coroutine
def listen_for_wemo_events(device_map: {str: WemoDevice}, home: Home):
    srv = yield from asyncio.get_event_loop().create_server(
        lambda: WemoEventHandler(device_map, home, debug=True, keep_alive=75),
        get_own_internal_ip_slow())
    callback_address = srv.sockets[0].getsockname()
    log.info('serving on {}'.format(callback_address))
    return callback_address
