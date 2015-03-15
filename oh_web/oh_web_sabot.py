#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import os
import shared.util as util

from bottle import route, run, template, static_file


@route('/')
def index():
    with open('templates/index.html', 'r') as fp:
        return template(fp.read(), **websocket_info)


RESOURCES = {
    'oh.js': 'build/oh_web.js',
    'home32.png': 'static/images/home32.png',
    'room32.png': 'static/images/room32.png',
    'hue32.png': 'static/images/hue32.png',
    'wemomotion32.png': 'static/images/wemomotion32.png',
}
@route('/resources/<name>')
def resources(name):
    return static_file(RESOURCES[name], root=os.getcwd())


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='OpenHouse interface server.')
    parser.add_argument('--address', '-a', default='0.0.0.0', type=str,
                        help="The address to listen on.")
    parser.add_argument('--port', '-p', default=8887, type=int,
                        help="The port to listen on.")
    util.add_common_args(parser)
    args = parser.parse_args()

    websocket_info = {
        'address': "ws://{}:{}/primus".format(args.home_address, args.home_port),
        'client_code': "http://{}:{}/primus/primus.js".format(args.home_address, args.home_port)
    }

    util.enable_logging(args.log_target, args.log_level)
    run(server='waitress', host=args.address, port=args.port)
