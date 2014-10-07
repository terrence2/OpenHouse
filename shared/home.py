# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging
import os
import select

import zmq

from threading import Thread, Lock


log = logging.getLogger('home')


class Query:
    def __init__(self, home, query):
        self.home = home
        self.query = query
        self.transforms = []

    def append(self, content):
        self.transforms.append({'method': 'append', 'args': [content]})
        return self

    def run(self):
        return self.home.execute_query(self)


class Home(Thread):
    """Sync binding to the oh_home server."""

    PollInterval = 500  # sec

    def __init__(self, lock: Lock):
        super().__init__()
        self.gil_ = lock
        self.quit_ = False

        self.poller_ = zmq.Poller()
        self.ctx_ = zmq.Context()
        self.query_sock_ = self.ctx_.socket(zmq.REQ)
        self.query_sock_.connect("ipc:///var/run/openhouse/home/query")

        self.event_socks = []

        # The poke socket.
        self.read_fd_, self.write_fd_ = os.pipe()
        self.poller_.register(self.read_fd_, select.POLLIN)

    def subscribe(self, name: str):
        sock = self.ctx_.socket(zmq.SUB)
        sock.connect("ipc:///var/run/openhouse/home/events")
        sock.setsockopt(zmq.SUBSCRIBE, name)
        self.poller_.register(sock, select.POLLIN)
        self.event_socks.append(sock)

    def query(self, query):
        return Query(self, query)

    def execute_query(self, query: Query):
        self.query_sock_.send_json({'type': 'query',
                                   'query': query.query,
                                   'transforms': query.transforms})
        result = self.query_sock_.recv_json()
        return result

    def poke(self):
        os.write(self.write_fd_, b'1')

    def exit(self):
        self.quit_ = True
        self.poke()

    def run(self):
        while not self.quit_:
            ready = self.poller_.poll(Home.PollInterval)
            if not ready:
                continue

            for (socket, event) in ready:
                if socket == self.read_fd_:
                    _ = os.read(self.read_fd_, 4096)
                    continue
                data = socket.recv()
                log.warning(data)
