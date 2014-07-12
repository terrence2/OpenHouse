#!/usr/bin/env python2
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from __future__ import print_function

import zmq

from ouimeaux.environment import Environment
from ouimeaux.signals import devicefound, statechange, receiver, subscription
from zmq.sugar import socket as zmq_socket
from threading import Lock, Thread
import logging
import json

log = logging.getLogger('wemo-bridge')

def enable_logging(level):
    # File logger captures everything.
    fh = logging.FileHandler('mcp-events.log')
    fh.setLevel(logging.DEBUG)

    # Console output level is configurable.
    ch = logging.StreamHandler()
    ch.setLevel(getattr(logging, level))

    # Set an output format.
    formatter = logging.Formatter('%(asctime)s:%(levelname)s:%(name)s:%(message)s')
    ch.setFormatter(formatter)
    fh.setFormatter(formatter)

    # Add handlers to root.
    root = logging.getLogger('')
    root.setLevel(logging.DEBUG)
    root.addHandler(ch)
    root.addHandler(fh)


class Network(Thread):
    ReplyPort = 31978
    BroadcastPort = 31979

    def __init__(self, lock):
        Thread.__init__(self)
        self.exiting_ = False

        # A lock to hold while executing response code.
        self.lock_ = lock

        self.ctx = zmq.Context()
        self.poller = zmq.Poller()
        self.devices = {}  # {zmq.socket: ???}

        self.pub_socket = self.ctx.socket(zmq.PUB)
        self.pub_socket.bind("tcp://*:{}".format(self.BroadcastPort))
        self.rep_socket = self.ctx.socket(zmq.REP)
        self.rep_socket.bind("tcp://*:{}".format(self.ReplyPort))

    def add_device(self, device):
        with self.lock_:
            self.devices[device.name] = device

    def broadcast(self, device, message):
        with self.lock_:
            wrapper = {'source': device.name, 'message': message}
            self.pub_socket.send(json.dumps(wrapper))

    def exit(self):
        with self.lock_:
            self.exiting_ = True
            sock = self.ctx.socket(zmq.REQ)
            sock.connect("tcp://127.0.0.1:{}".format(self.ReplyPort))
            sock.send(json.dumps({'target': 'none', 'type': 'exit'}))

    def run(self):
        log.info("Network thread starting...")

        while not self.exiting_:
            try:
                data = self.rep_socket.recv_json()
                log.info("Processing message: {}".format(data))
            except Exception:
                log.exception("failed to receive sensor message")
                continue

            with self.lock_:
                if self.exiting_:
                    return

                target = data['target']
                try:
                    device = self.devices[target]
                except KeyError:
                    log.error('unknown target device: {}'.format(target))
                    continue

                if data['type'] == 'get_state':
                    result = {'state': device.get_state()}
                    self.rep_socket.send_json(result)
                elif data['type'] == 'set_state':
                    state = bool(data['state'])
                    device.set_state(state)
                    result = {'state': device.get_state()}
                    self.rep_socket.send_json(result)
                else:
                    log.error("unhandled message type: {} for {}".format(data['type'], target))


if __name__ == '__main__':
    enable_logging('INFO')

    # Global state.
    gil = Lock()
    net = Network(gil)
    env = Environment(with_cache=False, bind="0.0.0.0:54321")

    @receiver(devicefound)
    def found(sender, **kwargs):
        log.info("Found device: {}".format(sender.name))
        net.add_device(sender)

    #@receiver(subscription)
    #def subscription(sender, **kwargs):
    #    log.info("Subscription Result: {} => {}: {}".format(sender.name, kwargs['type'], kwargs['value']))

    # Kick off threads.
    net.start()
    env.start()

    env.discover(10)
    log.info("Finished discovery.")

    @receiver(statechange)
    def state_update_event(sender, **kwargs):
        log.info("{} state is {state}".format(
            sender.name, state="on" if kwargs.get('state') else "off"))
        net.broadcast(sender, {'type': 'statechange', 'state': kwargs.get('state')})

    try:
        env.wait()
    except (KeyboardInterrupt, SystemExit):
        print("Goodbye!")

    net.exit()
    net.join(10)
