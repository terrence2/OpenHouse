#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import os

from threading import RLock

from sabot.home import Home

from bottle import route, run, template, static_file


def query_websock_info():
    h = Home((3, 0), RLock())
    ws = h.get_websocket_info()
    return ws


websocket_info = query_websock_info()


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


run(server='waitress', port=8887)
