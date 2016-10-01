#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from aiohttp import web
from pathlib import PurePosixPath as Path
from oh_shared.args import make_parser
from oh_shared.db import Tree, TreeError, make_connection
from oh_shared.log import enable_logging
import asyncio
import logging
import socket
import sys

log = logging.getLogger('oh_button')


async def update_ip_map(tree, ip_map):
    """Search for configured devices and nslookup to find their
       ip. Use this to build / update a reverse map from the ip
       to the path that should be updated.

       NOTE: This will get super-duper slow on large networks.
             Such networks will need to have have reverse ip mapping
             set up, and we will need to figure out how to use it.
    """
    radio_buttons = await tree.get_matching_files("/room/*/radio-button/*/state")
    for path in radio_buttons.keys():
        hostname = Path(path).parent.name
        try:
            ip = socket.gethostbyname(hostname)
        except socket.gaierror:
            log.warning("Device {}'s name does not map to an ip!".format(hostname))
            continue
        log.info("Mapping ip {} to path {}".format(ip, path))
        ip_map[ip] = path


def make_handler(tree: Tree, ip_map: {str: str} = {}):
    """Make listeners for aiohttp."""
    async def post(request):
        """Listen for POST requests. Update the proper path with the
           posted content."""
        data = await request.content.read()
        data = data.decode("UTF-8")
        peer = request.transport.get_extra_info('peername')
        if peer[0] not in ip_map:
            log.info("No {} in ip_map; re-querying names.".format(peer[0]))
            await update_ip_map(tree, ip_map)
        try:
            path = ip_map[peer[0]]
        except KeyError:
            return web.Response(status=404, reason="Unknown device")
        try:
            log.debug("Updating {} to {}".format(path, data))
            await tree.set_file(path, data)
        except TreeError as ex:
            return web.Response(status=502, reason=str(ex))
        return web.Response(status=200)

    return post


def main():
    parser = make_parser('A gateway for accepting button events into OpenHouse.')
    group = parser.add_argument_group("Where to listen for connections.")
    group.add_argument('-a', '--address', default='0.0.0.0',
                       help="The address to listen for REST on.")
    group.add_argument('-p', '--port', default=8090, type=int,
                       help="The port to listen for REST on.")
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    tree = asyncio.get_event_loop().run_until_complete(make_connection(args))

    app = web.Application()
    post_handler = make_handler(tree)
    paths = app.router.add_resource(r'/event')
    paths.add_route('POST', post_handler)
    log.info("Listening on '{}:{}'".format(args.address, args.port))
    web.run_app(app, host=args.address, port=args.port)


if __name__ == '__main__':
    sys.exit(main())
