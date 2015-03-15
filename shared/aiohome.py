# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import collections
import html
import itertools
import json
import logging
import websockets

from pprint import pformat
log = logging.getLogger('aiohome')


class NodeData:
    def __init__(self, node: dict):
        self.tagName = node['tagName']
        self.text = node['text']
        self.attrs = node['attrs']

        # Name is special: we need a name to even refer to the node, so we can assume it is present.
        self.name = node['attrs']['name']

    def __str__(self):
        if self.text:
            return "<{} {}>{}</{}>".format(self.tagName, self.attrs, self.text, self.tagName)
        return "<{} {}/>".format(self.tagName, self.attrs)


NodeMap = {str: NodeData}


class QueryGroup:
    def __init__(self, home):
        self.home = home
        self.query_group = []

    def query(self, query: str) -> 'Query':
        q = Query(self.home, query)
        self.query_group.append(q)
        return q

    @asyncio.coroutine
    def run(self) -> NodeMap:
        return self.home._execute_query_group(self.query_group)

    def __str__(self):
        parts = [str(q) for q in self.query_group]
        return "QueryGroup[\n{}]".format('\n'.join(parts))


class Query:
    def __init__(self, home, query: str):
        self.home = home
        self.query = query
        self.transforms = []  # {method: str, args: [str]}

    def after(self, content: str) -> 'Query':
        self.transforms.append({'method': 'after', 'args': [content]})
        return self

    def append(self, content: str) -> 'Query':
        self.transforms.append({'method': 'append', 'args': [content]})
        return self

    def attr(self, name: str, value: str) -> 'Query':
        args = [name] if value is None else [name, value]
        self.transforms.append({'method': 'attr', 'args': args})
        return self

    def css(self, name: str, value: str) -> 'Query':
        args = [name] if value is None else [name, value]
        self.transforms.append({'method': 'css', 'args': args})
        return self

    def empty(self) -> 'Query':
        self.transforms.append({'method': 'empty', 'args': []})
        return self

    def parent(self) -> 'Query':
        self.transforms.append({'method': 'parent', 'args': []})
        return self

    def children(self) -> 'Query':
        self.transforms.append({'method': 'children', 'args': []})
        return self

    @asyncio.coroutine
    def run(self) -> NodeMap:
        return self.home._execute_single_query(self)

    def __str__(self):
        xforms = [".{}({})".format(xform['method'], ', '.join(xform['args'])) for xform in self.transforms]
        return "$({}){}".format(self.query, ''.join(xforms))


class Home:
    def __init__(self, websock):
        self.websock = websock
        self.waiting = {}  # {int: Future}
        self.token = itertools.count(1)
        self.subscriptions = collections.defaultdict(list)  # {path: [coro]}

    @staticmethod
    def path_to_query(path: str):
        parts = path.strip('/').split('/')
        pieces = ['[name="{}"]'.format(part) for part in parts]
        return ' > '.join(pieces)

    def query(self, query: str) -> Query:
        return Query(self, query)

    def __call__(self, query: str) -> Query:
        return self.query(query)

    def group(self) -> QueryGroup:
        return QueryGroup(self)

    @asyncio.coroutine
    def listener(self):
        while True:
            raw = yield from self.websock.recv()
            if raw is None:
                log.critical("other end closed connection!")
                raise Exception("connection closed")
            frame = json.loads(raw)
            if 'token' in frame:
                token = frame['token']
                message = frame['message']
                assert token in self.waiting
                self.waiting[token].set_result(message)
            else:
                path = frame['path']
                message = NodeData(frame['message'])
                assert path in self.subscriptions, "unsubscribed path: {}".format(path)
                for coroutine in self.subscriptions[path]:
                    asyncio.async(coroutine(path, message))

    def _dispatch_message(self, message) -> NodeMap:
        token = next(self.token)
        self.waiting[token] = asyncio.Future()
        yield from self.websock.send(json.dumps({'token': token, 'message': message}))
        yield from self.waiting[token]
        result = self.waiting[token].result()
        del self.waiting[token]
        if 'error' in result:
            log.error("HOMe returned an error: {}".format(result['error']))
            raise Exception(result['error'])
        return {key: NodeData(val) for key, val in result.items()}

    @asyncio.coroutine
    def subscribe(self, path: str, coroutine: asyncio.coroutine):
        self.subscriptions[path].append(coroutine)
        return self._dispatch_message({'type': 'subscribe', 'target': path})

    def _execute_query_group(self, group: [Query]) -> NodeMap:
        msg = {'type': 'query', 'query_group': []}
        for query in group:
            msg['query_group'].append({'query': query.query, 'transforms': query.transforms})
        return self._dispatch_message(msg)

    def _execute_single_query(self, query: Query) -> NodeMap:
        msg = {'type': 'query', 'query_group': [
            {'query': query.query,
             'transforms': query.transforms}
        ]}
        return self._dispatch_message(msg)


@asyncio.coroutine
def connect(address: (str, int)) -> Home:
    while True:
        try:
            websock = yield from websockets.connect('ws://{}:{}/primus'.format(*address))
            break
        except ConnectionRefusedError:
            log.warn("Failed to connect, retrying in 0.5s")
            yield from asyncio.sleep(0.5)

    yield from websock.send(json.dumps({'token': 0, 'message': {'type': 'ping', 'ping': 'flimfniffle'}}))
    raw = yield from websock.recv()
    frame = json.loads(raw)
    assert frame['token'] == 0
    message = frame['message']
    assert message['pong'] == 'flimfniffle'
    home = Home(websock)
    asyncio.async(home.listener())
    return home

