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
class UnknownMessageType(ParseError): pass
class UnknownNodeType(ParseError): pass
class WrongFieldType(ParseError): pass

# The following must match up with tree.rs' TreeError enum.
class TreeError(DatabaseError): pass
class InvalidPathComponent(TreeError): pass
class MalformedPath(TreeError): pass
class NoSuchKey(TreeError): pass
class NoSuchNode(TreeError): pass
class NodeAlreadyExists(TreeError): pass
class DirectoryNotEmpty(TreeError): pass
class NodeContainsSubscriptions(TreeError): pass
class NotDirectory(TreeError): pass
class NotFile(TreeError): pass

# The following must match up with subscriptions.rs' SubscriptionError enum.
class SubscriptionError(DatabaseError): pass
class NoSuchSubscription(SubscriptionError): pass

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

        websock = None
        while not websock:
            try:
                ctx = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2)
                ctx.load_cert_chain(cert_chain, keyfile=key_file)
                ctx.load_verify_locations(cafile=ca_cert_chain)
                ctx.verify_mode = ssl.CERT_REQUIRED
                ctx.check_hostname = False
                websock = await websockets.connect('wss://{}:{}'.format(*address), ssl=ctx)
            except ConnectionRefusedError:
                log.warn("Failed to connect, retrying in 0.5s")
                await asyncio.sleep(0.5)

        await websock.send(json.dumps({'message_id': 1,
                                       'message_type': 'Ping',
                                       'message_payload': {
                                           'data': 'flimfniffle'}}))
        raw = await websock.recv()
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

                elif 'subscription_id' in message:
                    await self._handle_subscription_message(message)

                else:
                    assert False, "unknown message type received"

        except asyncio.CancelledError:
            return

    async def _dispatch_message(self, message_type: str, payload: dict) -> asyncio.Future:
        assert 'message_id' not in payload
        assert message_type in ("CreateNode", "ListDirectory", "SetFileContent",
                                "GetFileContent", "RemoveNode", "Subscribe", "Unsubscribe")
        message = {
            'message_id': next(self.message_id),
            'message_type': message_type,
            'message_payload': payload,
        }
        log.debug("sending message: {}".format(message['message_id']))
        response_future = self.awaiting_response[message['message_id']] = asyncio.Future()
        await self.websock.send(json.dumps(message))
        return response_future

    def _handle_response_message(self, message: dict):
        log.debug("got response: {}".format(message['message_id']))
        response_id = int(message['message_id'])
        del message['message_id']

        response_future = self.awaiting_response[response_id]
        del self.awaiting_response[response_id]

        if not response_future.cancelled():
            response_future.set_result(message)

    async def _handle_subscription_message(self, message: dict):
        sid = message['subscription_id']
        if sid not in self.subscriptions:
            log.critical("received unknown subscription: {}", sid)
            return
        try:
            cb = self.subscriptions[sid]
            await cb(message['path'], message['event'], message['context'])
        except Exception as e:
            log.critical("Handler for subscription id {} failed with exception:", sid)
            log.exception(e)

    @staticmethod
    def make_error(message: dict):
        assert message['status'] != 'Ok'
        exc_class = globals().get(message['status'], UnknownErrorType)
        raise exc_class(message['status'], message.get('context', "unknown"))

    # ########## LowLevel Async ##########
    async def create_node_async(self, node_type: str, parent_path: str, name: str):
        return await self._dispatch_message('CreateNode', {'type': node_type,
                                                           'parent_path': parent_path,
                                                           'name': name})

    async def remove_node_async(self, parent_path: str, name: str) -> asyncio.Future:
        return await self._dispatch_message('RemoveNode', {'parent_path': parent_path,
                                                           'name': name})

    async def list_directory_async(self, path: str) -> asyncio.Future:
        return await self._dispatch_message('ListDirectory', {'path': path})

    async def set_file_content_async(self, path: str, content: str) -> asyncio.Future:
        return await self._dispatch_message('SetFileContent', {'path': path, 'data': content})

    async def get_file_content_async(self, path: str) -> asyncio.Future:
        return await self._dispatch_message('GetFileContent', {'path': path})

    # Note that subscribe and unsubscribe do not have async varieties.
    # We do not guarantee message delivery order, so it is possible to
    # receive a subscription message before the subscription is
    # registered on this side of the channel.

    # ########## LowLevel Sync ##########
    async def create_node(self, node_type: str, parent_path: str, name: str):
        future = await self.create_node_async(node_type, parent_path, name)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)

    async def remove_node(self, parent_path: str, name: str):
        future = await self.remove_node_async(parent_path, name)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)

    async def list_directory(self, path: str) -> [str]:
        future = await self.list_directory_async(path)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)
        return result['children']

    async def set_file_content(self, path: str, content: str):
        future = await self.set_file_content_async(path, content)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)

    async def get_file_content(self, path: str) -> str:
        future = await self.get_file_content_async(path)
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)
        return result['data']

    async def subscribe(self, path: str, cb: callable) -> asyncio.Future:
        future = await self._dispatch_message('Subscribe', {'path': path})
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)
        self.subscriptions[result['subscription_id']] = cb
        return result['subscription_id']

    async def unsubscribe(self, sid: int) -> asyncio.Future:
        future = await self._dispatch_message('Unsubscribe', {'subscription_id': sid})
        result = await future
        if result['status'] != "Ok":
            raise self.make_error(result)
        del self.subscriptions[sid]

    # ########## High Level Interface ##########
    async def create_directory(self, parent_path: str, name: str):
        self.create_node("Directory", parent_path, name)

    async def create_file(self, parent_path: str, name: str):
        self.create_node("File", parent_path, name)


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


