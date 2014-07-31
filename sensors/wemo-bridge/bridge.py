#!/usr/bin/env python2
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from __future__ import print_function

import zmq

from ouimeaux.environment import Environment
from ouimeaux.signals import devicefound, statechange, receiver, subscription

from datetime import datetime
from select import select
from threading import Lock, Thread

import json
import logging
import os
import sys

log = logging.getLogger('wemo-bridge')


def enable_logging(level):
    # File logger captures everything.
    fh = logging.FileHandler('bridge.log')
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


class Watchdog(Thread):
    def __init__(self, lock, network):
        Thread.__init__(self)
        self.lock_ = lock
        self.network_ = network
        self.read_fd_, self.write_fd_ = os.pipe()
        self.wants_exit_ = False

    def exit(self):
        self.wants_exit_ = True
        os.write(self.write_fd_, b'0')

    def run(self):
        while not self.wants_exit_:
            readable = select([self.read_fd_], [], [], 10.)
            if self.read_fd_ in readable:
                buf = os.read(self.read_fd_, 1)
                assert buf == b'0'
                if self.wants_exit_:
                    return

            log_string = ''
            maximum = 0
            with self.lock_:
                now = datetime.now()
                devnames = sorted(self.network_.devices_.keys())
                for name in devnames:
                    if not name.startswith('wemomotion'):
                        continue
                    dt = now - self.network_.devices_[name].last_update
                    if dt.seconds > maximum:
                        maximum = dt.seconds
                    log_string += '{}:{} '.format(name[len('wemomotion-'):], dt.seconds)
            log.info(log_string)

            if maximum > 45:
                log.critical("DETECTED FRAMEWORK DEATH")
                os._exit(3)


class NetworkDevice(object):
    def __init__(self, device):
        self.device = device
        self.last_update = datetime.now()


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
        self.devices_ = {}  # {str: NetworkDevice}

        self.pub_socket = self.ctx.socket(zmq.PUB)
        self.pub_socket.bind("tcp://*:{}".format(self.BroadcastPort))
        self.rep_socket = self.ctx.socket(zmq.REP)
        self.rep_socket.bind("tcp://*:{}".format(self.ReplyPort))

    def add_device(self, device):
        with self.lock_:
            self.devices_[device.name] = NetworkDevice(device)

    def broadcast(self, device, message):
        with self.lock_:
            self.devices_[device.name].last_update = datetime.now()
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
                    net_device = self.devices_[target]
                except KeyError:
                    log.error('unknown target device: {}'.format(target))
                    continue

                if data['type'] == 'get_state':
                    result = {'state': net_device.device.get_state()}
                    self.rep_socket.send_json(result)
                elif data['type'] == 'set_state':
                    state = bool(data['state'])
                    net_device.last_update = datetime.now()
                    result = net_device.device.set_state(state)
                    self.rep_socket.send_json({'result': result})
                else:
                    log.error("unhandled message type: {} for {}".format(data['type'], target))


if __name__ == '__main__':
    enable_logging('INFO')

    # Global state.
    gil = Lock()
    net = Network(gil)
    wd = Watchdog(gil, net)
    env = Environment(with_cache=False, bind="0.0.0.0:54321")

    @receiver(devicefound)
    def found(sender, **kwargs):
        log.info("Found device: {}".format(sender.name))
        net.add_device(sender)

    @receiver(subscription)
    def subscription(sender, **kwargs):
        log.debug("Subscription Result: {} => {}: {}".format(sender.name, kwargs['type'], kwargs['value']))

    # Kick off threads.
    net.start()
    env.start()

    env.discover(10)
    log.info("Finished discovery.")
    log.info("Discovered {} devices:".format(len(net.devices_)))
    devnames = sorted(net.devices_.keys())
    for name in devnames:
        log.info("-- {}".format(name))

    @receiver(statechange)
    def state_update_event(sender, **kwargs):
        log.debug("{} state is {state}".format(
            sender.name, state="on" if kwargs.get('state') else "off"))
        net.broadcast(sender, {'type': 'statechange', 'state': kwargs.get('state')})

    try:
        wd.start()
        env.wait()
    except (KeyboardInterrupt, SystemExit):
        log.debug("Goodbye!")

    wd.exit()
    net.exit()
    wd.join(10)
    net.join(10)
