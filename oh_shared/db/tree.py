# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import capnp
import json
import itertools
import logging
import os.path
import ssl
import websockets


# Explicit is better than implicit.
capnp.remove_import_hook()
messages = capnp.load('oh_shared/db/messages.capnp')


# Re-export some deeply nested enums so that users don't have to worry about message structural details.
NodeType = messages.CreateNodeRequest.NodeType
EventKind = messages.EventKind


# Tag all log messages that come from this module.
log = logging.getLogger('db.tree')


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
class NonAbsolutePath(TreeError): pass
class Dotfile(TreeError): pass
class EmptyComponent(TreeError): pass
class InvalidCharacter(TreeError): pass
class InvalidWhitespace(TreeError): pass
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
        assert len(self.awaiting_response) == 0
        await self.websock.close()
        await self.listener_task

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

        request = messages.ClientRequest.new_message(id=1, ping=messages.PingRequest.new_message(data='flimfniffle'))
        await websock.send(request.to_bytes())
        raw = await websock.recv()
        message = messages.ServerMessage.from_bytes(raw)
        assert message.which() == 'response'
        assert message.response.id == 1
        assert message.response.which() == 'ping'
        assert message.response.ping.pong == 'flimfniffle'
        return Tree(websock)

    async def _listener(self):
        while True:
            try:
                raw = await self.websock.recv()
            except websockets.exceptions.ConnectionClosed:
                log.critical("other end closed connection!")
                return

            server_message = messages.ServerMessage.from_bytes(raw)
            if server_message.which() == 'response':
                self._handle_response_message(server_message.response)
            elif server_message.which() == 'event':
                await self._handle_subscription_message(server_message.event)
            else:
                assert False, "unknown message type received"

    async def _dispatch_message(self, **kwargs) -> asyncio.Future:
        message_id = next(self.message_id)
        assert message_id not in self.awaiting_response
        request = messages.ClientRequest.new_message(id=message_id, **kwargs)
        response_future = self.awaiting_response[message_id] = asyncio.Future()
        await self.websock.send(request.to_bytes())
        return response_future

    @staticmethod
    def _get_concrete_response(response: messages.ServerResponse):
        for name in ('ok', 'getFileContent', 'listDirectory', 'subscribe', 'ping'):
            if response.which() == name:
                return getattr(response, name)
        raise NotImplementedError("unknown response type: {}".format(response.which()))

    def _handle_response_message(self, response: messages.ServerResponse):
        log.debug("got response: {}".format(response.id))
        response_future = self.awaiting_response.pop(response.id)
        if response.which() == 'error':
            err = self.make_error(response.error)
            response_future.set_exception(err)
            return
        concrete = self._get_concrete_response(response)
        if not response_future.cancelled():
            response_future.set_result(concrete)

    async def _handle_subscription_message(self, event: messages.SubscriptionMessage):
        sid = event.subscriptionId
        if sid not in self.subscriptions:
            log.critical("received unknown subscription: {}", sid)
            return
        try:
            cb = self.subscriptions[sid]
            await cb(event.paths, event.kind, event.context)
        except Exception as e:
            log.critical("Handler for subscription id {} failed with exception:", sid)
            log.exception(e)

    @staticmethod
    def make_error(error: messages.ErrorResponse):
        exc_class = globals().get(error.name, UnknownErrorType)
        return exc_class(error.name, error.context)

    # ########## LowLevel Async ##########
    async def create_node_async(self, node_type: str, parent_path: str, name: str):
        return await self._dispatch_message(createNode=messages.CreateNodeRequest.new_message(parentPath=parent_path,
                                                                                              nodeType=node_type,
                                                                                              name=name))

    async def remove_node_async(self, parent_path: str, name: str) -> asyncio.Future:
        return await self._dispatch_message(removeNode=messages.RemoveNodeRequest.new_message(parentPath=parent_path,
                                                                                              name=name))

    async def list_directory_async(self, path: str) -> asyncio.Future:
        return await self._dispatch_message(listDirectory=messages.ListDirectoryRequest.new_message(path=path))

    async def set_file_content_async(self, glob: str, content: str) -> asyncio.Future:
        return await self._dispatch_message(setFileContent=messages.SetFileContentRequest.new_message(glob=glob,
                                                                                                      data=content))

    async def get_file_content_async(self, path: str) -> asyncio.Future:
        return await self._dispatch_message(getFileContent=messages.GetFileContentRequest.new_message(path=path))

    # Note that subscribe and unsubscribe do not have async varieties.
    # We do not guarantee message delivery order, so it is possible to
    # receive a subscription message before the subscription is
    # registered on this side of the channel.

    # ########## LowLevel Sync ##########
    async def create_node(self, node_type: str, parent_path: str, name: str):
        future = await self.create_node_async(node_type, parent_path, name)
        await future

    async def remove_node(self, parent_path: str, name: str):
        future = await self.remove_node_async(parent_path, name)
        await future

    async def list_directory(self, path: str) -> [str]:
        future = await self.list_directory_async(path)
        result = await future
        return result.children

    async def set_file_content(self, path: str, content: str):
        future = await self.set_file_content_async(path, content)
        await future

    async def get_file_content(self, path: str) -> str:
        future = await self.get_file_content_async(path)
        result = await future
        return result.data

    async def subscribe(self, glob: str, cb: callable) -> asyncio.Future:
        future = await self._dispatch_message(subscribe=messages.SubscribeRequest.new_message(glob=glob))
        result = await future
        self.subscriptions[result.subscriptionId] = cb
        return result.subscriptionId

    async def unsubscribe(self, sid: int) -> asyncio.Future:
        future = await self._dispatch_message(unsubscribe=messages.UnsubscribeRequest.new_message(subscriptionId=sid))
        await future
        del self.subscriptions[sid]

    # ########## High Level Interface ##########
    async def create_directory(self, parent_path: str, name: str):
        await self.create_node(NodeType.directory, parent_path, name)

    async def create_file(self, parent_path: str, name: str):
        await self.create_node(NodeType.file, parent_path, name)
