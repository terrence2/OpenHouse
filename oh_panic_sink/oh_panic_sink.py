#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from aiohttp import web
from oh_shared.args import make_parser
from oh_shared.log import enable_logging
import logging
import sys

log = logging.getLogger('oh_panic_sink')


def make_handler():
    """Make listeners for aiohttp."""
    async def post(request):
        """Listen for POST requests and print the content as an error."""
        data = await request.content.read()
        log.error(data.decode('UTF-8'))
        return web.Response(status=200)

    return post


def main():
    parser = make_parser('A helper to print OpenActuator panic messages into the OpenHouse console.')
    group = parser.add_argument_group("Where to listen for connections.")
    group.add_argument('-a', '--address', default='0.0.0.0',
                       help="The address to listen for panics on.")
    group.add_argument('-b', '--bind', default=6666, type=int,
                       help="The port to listen for panics on.")
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    app = web.Application()
    post_handler = make_handler()
    paths = app.router.add_resource(r'/event')
    paths.add_route('POST', post_handler)
    log.info("Listening on '{}:{}'".format(args.address, args.bind))
    web.run_app(app, host=args.address, port=args.bind)


if __name__ == '__main__':
    sys.exit(main())
