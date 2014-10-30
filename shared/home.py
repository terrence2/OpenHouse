# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging
import html
import json
import os
import select

from mcp.util import ExitableThread

import zmq

from threading import RLock


log = logging.getLogger('home')


class QueryGroup:
    def __init__(self, home):
        self.home = home
        self.query_group = []

    def query(self, query: str):
        q = Query(self.home, query)
        self.query_group.append(q)
        return q

    def add_or_empty(self, parent: str, node: str, content: str):
        if self.home.query(node).run():
            self.query(node).empty()
        else:
            self.query(parent).append(content)

    def reflect_as_properties(self, query: str, props: dict):
        for key, value in props.items():
            assert isinstance(key, str)
            key = key.replace(' ', '_')

            if isinstance(value, list):
                property_content = '<div kind="property-group" name="{}"></div>'.format(key)
                property_query = query + " > [kind='property-group'][name='{}']".format(key)
                self.query(query).append(property_content)
                self.reflect_as_properties(property_query, {str(i): v for i, v in enumerate(value)})
            elif isinstance(value, dict):
                property_content = '<div kind="property-group" name="{}"></div>'.format(key)
                property_query = query + " > [kind='property-group'][name='{}']".format(key)
                self.query(query).append(property_content)
                self.reflect_as_properties(property_query, value)
            else:
                content = '<div kind="property" name="{}" {}="{}"></div>'.format(key, key, html.escape(str(value)))
                self.query(query).append(content)

    def run(self):
        return self.home.execute_query_group(self.query_group)

    def __str__(self):
        parts = [str(q) for q in self.query_group]
        return "QueryGroup[\n{}]".format('\n'.join(parts))


class Query:
    def __init__(self, home, query: str):
        self.home = home
        self.query = query
        self.transforms = []  # {method: str, args: [str]}

    def after(self, content: str):
        self.transforms.append({'method': 'after', 'args': [content]})
        return self

    def append(self, content: str):
        self.transforms.append({'method': 'append', 'args': [content]})
        return self

    def attr(self, name: str, value: str):
        args = [name] if value is None else [name, value]
        self.transforms.append({'method': 'attr', 'args': args})
        return self

    def css(self, name: str, value: str):
        args = [name] if value is None else [name, value]
        self.transforms.append({'method': 'css', 'args': args})
        return self

    def empty(self):
        self.transforms.append({'method': 'empty', 'args': []})
        return self

    def parent(self):
        self.transforms.append({'method': 'parent', 'args': []})
        return self

    def children(self):
        self.transforms.append({'method': 'children', 'args': []})
        return self

    def run(self):
        return self.home.execute_single_query(self)

    def __str__(self):
        xforms = [".{}({})".format(xform['method'], ', '.join(xform['args'])) for xform in self.transforms]
        return "$({}){}".format(self.query, ''.join(xforms))


class Home(ExitableThread):
    """Sync binding to the oh_home server."""

    PollInterval = 500  # sec

    def __init__(self, version: (int, int), lock: RLock):
        super().__init__()
        self.required_version = version
        self.gil_ = lock
        self.quit_ = False

        self.poller_ = zmq.Poller()
        self.ctx_ = zmq.Context()

        self.query_sock_ = self.ctx_.socket(zmq.REQ)
        self.query_sock_.connect("ipc:///var/run/openhouse/home/query")
        self.check_version()

        # Initial state is filtering out all messages.
        self.subscription_sock_ = self.ctx_.socket(zmq.SUB)
        self.subscription_sock_.connect("ipc:///var/run/openhouse/home/events")
        self.poller_.register(self.subscription_sock_, select.POLLIN)
        self.subscriptions_ = {}  # { subscription_text: callback }

        # The poke socket.
        self.read_fd_, self.write_fd_ = os.pipe()
        self.poller_.register(self.read_fd_, select.POLLIN)

    def check_version(self):
        self.query_sock_.send_json({'type': 'ping', 'ping': 'hello'})
        result = self.query_sock_.recv_json()
        assert result['pong'] == 'hello'
        assert result['version']['major'] == self.required_version[0]
        assert result['version']['minor'] >= self.required_version[1]

    def subscribe(self, name: str, callback: callable):
        self.subscription_sock_.setsockopt_string(zmq.SUBSCRIBE, name)
        self.subscriptions_[name] = callback

    def path_to_query(self, path: str):
        parts = path.strip('/').split('/')
        pieces = ["[name='{}']".format(part) for part in parts]
        return ' > '.join(pieces)

    def html(self) -> str:
        with self.gil_:
            self.query_sock_.send_json({'type': 'html'})
            result = self.query_sock_.recv_json()
        return result['html']

    def query(self, query):
        return Query(self, query)

    def group(self):
        return QueryGroup(self)

    def execute_query_group(self, group: [Query]) -> {str: str}:
        msg = {'type': 'query', 'query_group': []}
        for query in group:
            msg['query_group'].append({'query': query.query, 'transforms': query.transforms})
        with self.gil_:
            self.query_sock_.send_json(msg)
            result = self.query_sock_.recv_json()
        return result

    def execute_single_query(self, query: Query) -> {str: str}:
        with self.gil_:
            self.query_sock_.send_json({'type': 'query',
                                        'query_group': [
                                            {'query': query.query,
                                             'transforms': query.transforms}
                                        ]})
            result = self.query_sock_.recv_json()
        return result

    def poke(self):
        os.write(self.write_fd_, b'1')

    def exit(self):
        self.quit_ = True
        self.poke()

    def handle_poke(self, socket) -> bool:
        if socket == self.read_fd_:
            _ = os.read(self.read_fd_, 4096)
            return True
        return False

    def handle_event(self, socket):
        assert socket == self.subscription_sock_
        data = socket.recv()
        target, _, serialized = data.decode('UTF-8').partition(' ')
        # Note: subscriptions will send anything matching the prefix, which is a common occurence with the paths here.
        #       The fact that our map is on subscript names means that we automatically filter anything we don't want.
        if target in self.subscriptions_:
            log.debug("received subscription: {}".format(data))
            deserialized = json.loads(serialized)
            callback = self.subscriptions_[target]
            with self.gil_:
                callback(target, deserialized)
        else:
            log.debug("filtered prefix subscription: {}".format(data))

    def run(self):
        while not self.quit_:
            ready = self.poller_.poll(Home.PollInterval)
            if not ready:
                continue

            for (socket, event) in ready:
                if self.handle_poke(socket):
                    continue
                self.handle_event(socket)
