# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import json
import itertools
import logging
import os.path
import ssl
import websockets


log = logging.getLogger('db')


class DatabaseError(Exception):
    def __init__(self, name, context):
        super().__init__("{}".format(context))
        self.name = name
        self.context = context


# The following must match up with message.rs' ParseError enum.
class ParseError(DatabaseError): pass
class IdOutOfRange(ParseError): pass
class MissingField(ParseError): pass
class WrongFieldType(ParseError): pass
class UnknownType(ParseError): pass

# The following must match up with tree.rs' TreeError enum.
class TreeError(DatabaseError): pass
class InvalidPathComponent(TreeError): pass
class MalformedPath(TreeError): pass
class NoSuchNode(TreeError): pass
class NoSuchSubscription(TreeError): pass
class NodeAlreadyExists(TreeError): pass
class NodeContainsChildren(TreeError): pass
class NodeContainsSubscriptions(TreeError): pass
class NodeContainsData(TreeError): pass

# This will get thrown if we receive a string we do not expect
# as the status.
class UnknownErrorType(DatabaseError): pass


class Tree:
    """
    Represents an oh_db tree.
    """
    def __init__(self, websock):
        self.websock = websock
        self.awaiting_response = {}
        self.subscriptions = {}
        self.message_id = itertools.count(2)

        self.listener_task = asyncio.ensure_future(self._listener())

    async def close(self):
        await self.websock.close()
        self.listener_task.cancel()
        asyncio.wait(self.listener_task)

    @staticmethod
    async def connect(address: (str, int),
                      ca_cert_chain: str,
                      cert_chain: str,
                      key_file: str) -> 'Tree':
        assert os.path.exists(ca_cert_chain)
        assert os.path.exists(cert_chain)
        assert os.path.exists(key_file)
        while True:
            try:
                ctx = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2)
                ctx.load_cert_chain(cert_chain, keyfile=key_file)
                ctx.load_verify_locations(cafile=ca_cert_chain)
                ctx.verify_mode = ssl.CERT_REQUIRED
                ctx.check_hostname = False
                websock = await websockets.connect('wss://{}:{}'.format(*address), ssl=ctx)
                break
            except ConnectionRefusedError:
                log.warn("Failed to connect, retrying in 0.5s")
                await asyncio.sleep(0.5)

        await websock.send(json.dumps({'message_id': 1,
                                       'type': 'Ping',
                                       'data': 'flimfniffle'}))
        raw = await websock.recv()
        print("raw: {}".format(raw))
        response = json.loads(raw)
        assert response['message_id'] == 1
        assert response['pong'] == 'flimfniffle'
        return Tree(websock)

    async def _listener(self):
        try:
            while True:
                try:
                    raw = await self.websock.recv()
                except websockets.exceptions.ConnectionClosed:
                    log.critical("other end closed connection!")
                    return
                message = json.loads(raw)

                if 'message_id' in message:
                    self._handle_response_message(message)

                elif 'layout_subscription_id' in message:
                    await self._handle_subscription_message(message)

        except asyncio.CancelledError:
            return

    async def _dispatch_message(self, message: dict) -> asyncio.Future:
        assert 'message_id' not in message
        send_id = next(self.message_id)
        message['message_id'] = send_id
        response_future = self.awaiting_response[send_id] = asyncio.Future()
        await self.websock.send(json.dumps(message))
        return response_future

    def _handle_response_message(self, message: dict):
        response_id = int(message['message_id'])
        del message['message_id']

        response_future = self.awaiting_response[response_id]
        del self.awaiting_response[response_id]

        if not response_future.cancelled():
            response_future.set_result(message)

    async def _handle_subscription_message(self, message: dict):
        layout_sid = message['layout_subscription_id']
        if layout_sid not in self.subscriptions:
            log.critical("received unknown subscription: {}", layout_sid)
            return
        try:
            cb = self.subscriptions[layout_sid]
            await cb(message['path'], message['event'], message['name'])
        except Exception as e:
            log.critical("Handler for subscription id {} failed with exception:", layout_sid)
            log.exception(e)

    @staticmethod
    def make_error(message: dict):
        assert message['status'] != 'Ok'
        exc_class = globals().get(message['status'], UnknownErrorType)
        raise exc_class(message['status'], message.get('context', "unknown"))

    async def create_child_async(self, parent_path: str, name: str) -> asyncio.Future:
        return await self._dispatch_message({'type': 'CreateChild',
                                             'parent_path': parent_path,
                                             'name': name})

    async def create_child(self, parent_path: str, name: str):
        future = await self.create_child_async(parent_path, name)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)

    async def remove_child_async(self, parent_path: str, name: str) -> asyncio.Future:
        return await self._dispatch_message({'type': 'RemoveChild',
                                             'parent_path': parent_path,
                                             'name': name})

    async def remove_child(self, parent_path: str, name: str):
        future = await self.remove_child_async(parent_path, name)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)

    async def list_children_async(self, path: str) -> asyncio.Future:
        return await self._dispatch_message({'type': 'ListChildren',
                                             'path': path})

    async def list_children(self, path: str) -> [str]:
        future = await self.list_children_async(path)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)
        return result['children']

    async def subscribe_children(self, path: str, cb: callable) -> int:
        future = await self._dispatch_message({'type': 'SubscribeLayout',
                                               'path': path})
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)
        self.subscriptions[result['layout_subscription_id']] = cb
        return result['layout_subscription_id']

    async def unsubscribe_children(self, layout_sid: int):
        future = await self._dispatch_message({'type': 'UnsubscribeLayout',
                                               'layout_subscription_id': layout_sid})
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)


class Connection:
    """
    An async context manager to create and clean up a Tree connection.
    """
    def __init__(self, address: (str, int), ca_cert_chain: str, cert_chain: str, key_file: str):
        self.address = address
        self.ca_cert_chain = ca_cert_chain
        self.cert_chain = cert_chain
        self.key_file = key_file
        self.connection = None

    async def __aenter__(self):
        self.connection = await Tree.connect(self.address, self.ca_cert_chain,
                                             self.cert_chain, self.key_file)
        return self.connection

    async def __aexit__(self, exc, *args):
        await self.connection.close()


