#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from aiohttp import web
import asyncio
import logging
from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging
from oh_shared.home import Home, NodeData

log = logging.getLogger('oh_sun')


def make_device_handler(home: Home):
    """Keep an open connection to home while we process transactions."""
    def _get_path_and_query(request):
        dwelling = request.match_info['dwelling']
        room = request.match_info['room']
        device = request.match_info['device']
        return ('/{}/{}/{}'.format(dwelling, room, device),\
                home.query("home[name='{}'] room[name='{}'] [name='{}']".format(dwelling, room, device)))

    @asyncio.coroutine
    def get(request):
        path, query = _get_path_and_query(request)
        data = yield from query.run()
        if not data:
            return web.Response(status=404)
        node = data[path]
        value = node.attrs.get(request.match_info['attr'], None)
        if value is None:
            return web.Response(status=404)
        return web.Response(body=str(value).encode('utf-8'))

    @asyncio.coroutine
    def post(request):
        body_raw = yield from request.read()
        value = body_raw.decode('utf-8')
        attr = request.match_info['attr']
        path, query = _get_path_and_query(request)
        node = yield from query.attr(attr, value).run()
        if not node:
            return web.Response(status=405)
        return web.Response(status=200)

    return get, post


@asyncio.coroutine
def main():
    args = parse_default_args('Respond to sunrise and sunset states with dramatic fades.')
    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))
    return home


if __name__ == '__main__':
    home = asyncio.get_event_loop().run_until_complete(main())
    app = web.Application()
    get_handler, post_handler = make_device_handler(home)
    app.router.add_route('GET', '/device/{dwelling}/{room}/{device}/{attr}', get_handler)
    app.router.add_route('POST', '/device/{dwelling}/{room}/{device}/{attr}', post_handler)
    web.run_app(app, host='0.0.0.0', port=8888)
