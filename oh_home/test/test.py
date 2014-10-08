#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import os.path
import select
import subprocess

from pprint import pprint

import zmq

nodeproc = subprocess.Popen(['node', 'build/main.js', 'test/test.html'])

ctx = zmq.Context()
home = ctx.socket(zmq.REQ)
home.connect("ipc:///var/run/openhouse/home/query")


def test_ping():
    home.send_json({'type': 'ping', 'ping': "foo"})
    data = home.recv_json()
    assert data['pong'] == "foo"
    print("ping: ok")


def test_unknown():
    home.send_json({'type': 'unknown'})
    data = home.recv_json()
    assert 'unrecognized' in data['error'].lower()
    print("unknown: ok")


def test_query():
    # Basic query for all lights.
    home.send_json({'type': 'query', 'query': "[kind='hue']", 'transforms': []})
    data = home.recv_json()
    for i in range(3):
        for j in range(3):
            name = "/root/room{}/light{}".format(i, j)
            assert name in data
            assert data[name]['kind'] == 'hue'
            assert data[name]['name'].startswith('light')

    # Query wemomotion named 0.
    home.send_json({'type': 'query', 'query': "[kind='wemomotion'][name='motion0']"})
    data = home.recv_json()
    for i in range(3):
        name = "/root/room{}/motion0".format(i)
        assert name in data
        assert data[name]['kind'] == 'wemomotion'
        assert data[name]['name'].startswith('motion')

    # Query wemomotion named 0. And set value to on.
    home.send_json({'type': 'query', 'query': "[kind='wemomotion'][name='motion0']",
        'transforms': [{'method': 'attr', 'args': ['value', 'on']}]})
    data = home.recv_json()
    for i in range(3):
        name = "/root/room{}/motion0".format(i)
        assert name in data
        assert data[name]['kind'] == 'wemomotion'
        assert data[name]['name'].startswith('motion')
        assert data[name]['value'] == 'on'

    # Query all wemomotions and verify we're still 'on'.
    home.send_json({'type': 'query', 'query': "[kind='wemomotion']", 'transforms': []})
    data = home.recv_json()
    for i in range(3):
        for j in range(3):
            name = "/root/room{}/motion{}".format(i, j)
            assert name in data
            assert data[name]['kind'] == 'wemomotion'
            assert data[name]['name'].startswith('motion')
            if j == 0:
                assert data[name]['value'] == 'on'

    print("query: ok")
    

def test_subscribe():
    sub = ctx.socket(zmq.SUB)
    sub.connect("ipc:///var/run/openhouse/home/events")
    sub.setsockopt(zmq.SUBSCRIBE, b"/root/room1/motion1")
    poller = zmq.Poller()
    poller.register(sub, select.POLLIN)

    while not poller.poll(0.1):
        home.send_json({'type': 'query', 'query': "[kind='wemomotion'][name='motion1']",
            'transform': [{'method': 'attr', 'args': ['value', 'on']}]})
        data = home.recv_json()

    data = sub.recv()
    pprint(data)

    print("subscribe: ok")


test_ping()
test_unknown()
test_query()
test_subscribe()

home.close()
nodeproc.terminate()
nodeproc.wait()
