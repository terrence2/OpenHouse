#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from aiohttp import web
from oh_shared.args import add_common_args
from oh_shared.db import Tree, NotDirectory, PathError, TreeError
from oh_shared.log import enable_logging
import argparse
import asyncio
import logging
import sys

log = logging.getLogger('oh_rest')


async def make_connection(args):
    tree = await Tree.connect((args.home_address, args.home_port),
                              args.ca_chain, args.certificate, args.private_key)
    return tree


def make_handler(tree: Tree):
    async def get(request):
        path = '/' + request.match_info['path']

        try:
            entries = await tree.list_directory(path)
            return web.json_response({'type': 'Directory', 'entries': entries})
        except NotDirectory:
            pass  # Fall back to try as a file.
        except TreeError as ex:
            return web.Response(status=502, reason=str(ex))
        except PathError as ex:
            return web.Response(status=502, reason=str(ex))

        try:
            content = await tree.get_file(path)
            return web.json_response({'type': 'File', 'data': content})
        except TreeError as ex:
            return web.Response(status=502, reason=str(ex))

    async def post(request):
        path = '/' + request.match_info['path']
        data = await request.content.read()
        try:
            await tree.set_file(path, data)
            return web.Response(status=200)
        except TreeError as ex:
            return web.Response(status=502, reason=str(ex))

    return get, post


def main():
    desc = 'A REST gateway for interacting with OpenHouse over HTTP.'
    parser = argparse.ArgumentParser(description=desc)
    add_common_args(parser)
    group = parser.add_argument_group("REST specific args")
    group.add_argument('-a', '--address', default='0.0.0.0',
                       help="The address to listen for REST on.")
    group.add_argument('-p', '--port', default=8080, type=int,
                       help="The port to listen for REST on.")
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    tree = asyncio.get_event_loop().run_until_complete(make_connection(args))

    app = web.Application()
    get_handler, post_handler = make_handler(tree)
    paths = app.router.add_resource(r'/{path:[^{}]+}')
    paths.add_route('GET', get_handler)
    paths.add_route('POST', post_handler)
    log.info("Listening on '{}:{}'".format(args.address, args.port))
    web.run_app(app, host=args.address, port=args.port)


if __name__ == '__main__':
    sys.exit(main())
