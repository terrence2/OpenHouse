# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging
import os
import select

import zmq

from threading import Thread, Lock


log = logging.getLogger('home')


class Home(Thread):
    """Sync binding to the oh_home server."""

    PollInterval = 500  # sec

    def __init__(self, lock: Lock):
        super().__init__()
        self.gil = lock
        self.quit_ = False

        self.poller = zmq.Poller()
        self.ctx = zmq.Context()
        self.query_sock = self.ctx.socket(zmq.REQ)
        self.query_sock.connect("ipc://var/run/openhouse/home/query")

        self.event_socks = []

        # The poke socket.
        self.read_fd_, self.write_fd_ = os.pipe()
        self.poller.register(self.read_fd_, select.POLLIN)

    def subscribe(self, name: str):
        sock = self.ctx.socket(zmq.SUB)
        sock.connect("ipc://var/run/openhouse/home/events")
        sock.setsockopt(zmq.SUBSCRIBE, name)
        self.poller.register(sock, select.POLLIN)
        self.event_socks.append(sock)

    def query(self, query: str, transforms: []):
        self.query_sock.send_json({'type': 'query',
                                   'query': query,
                                   'transforms': transforms})
        result = self.query_sock.recv_json()
        return result

    def poke(self):
        os.write(self.write_fd_, b'1')

    def exit(self):
        self.quit_ = True
        self.poke()

    def run(self):
        while not self.quit_:
            ready = self.poller.poll(Home.PollInterval)
            if not ready:
                continue

            for (socket, event) in ready:
                if socket == self.read_fd_:
                    _ = os.read(self.read_fd_, 4096)
                    continue
                data = socket.recv()
                log.warning(data)
