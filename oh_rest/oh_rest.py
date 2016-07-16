#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from aiohttp import web
from oh_shared.args import parse_default_args
from oh_shared.db import Tree, NotDirectory, TreeError
from oh_shared.home import Home, NodeData
from oh_shared.log import enable_logging
import asyncio
import json
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

        try:
            content = await tree.get_file_content(path)
            return web.json_response({'type': 'File', 'data': content})
        except TreeError as ex:
            return web.Response(status=502, reason=str(ex))

    async def post(request):
        path = '/' + request.match_info['path']
        try:
            await tree.set_file_content(path)
            return web.Response(status=200)
        except TreeError as ex:
            return web.Response(status=502, reason=str(ex))

    return get, post


def main():
    args = parse_default_args('A REST gateway for interacting with OpenHouse over HTTP.')
    enable_logging(args.log_target, args.log_level)

    tree = asyncio.get_event_loop().run_until_complete(make_connection(args))

    app = web.Application()
    get_handler, post_handler = make_handler(tree)
    paths = app.router.add_resource(r'/{path:[^{}]+}')
    paths.add_route('GET', get_handler)
    paths.add_route('POST', post_handler)
    web.run_app(app, host='0.0.0.0', port=8889)


if __name__ == '__main__':
    sys.exit(main())
