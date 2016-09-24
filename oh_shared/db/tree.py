# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import capnp
import itertools
import logging
import os.path
import ssl
import websockets


# Explicit is better than implicit.
capnp.remove_import_hook()
messages = capnp.load('oh_shared/db/messages.capnp')


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
class NoSuchKey(TreeError): pass
class NoSuchNode(TreeError): pass
class NodeAlreadyExists(TreeError): pass
class DirectoryNotEmpty(TreeError): pass
class NodeContainsSubscriptions(TreeError): pass
class NotDirectory(TreeError): pass
class NotFile(TreeError): pass
class FormulaInputNotFound(TreeError): pass

# The following should match up with path.rs' PathError enum.
class PathError(DatabaseError): pass
class NonAbsolutePath(PathError): pass
class Dotfile(PathError): pass
class EmptyComponent(PathError): pass
class InvalidCharacter(PathError): pass
class InvalidControlCharacter(PathError): pass
class InvalidWhitespaceCharacter(PathError): pass
class InvalidGlobCharacter(PathError): pass
class UnreachablePattern(PathError): pass
class NoParent(PathError): pass
class NoBasename(PathError): pass

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
        self.subscription_tasks = []

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

            # Process the message.
            server_message = messages.ServerMessage.from_bytes(raw)
            if server_message.which() == 'response':
                self._handle_response_message(server_message.response)
            elif server_message.which() == 'event':
                task = asyncio.ensure_future(self._handle_subscription_message(server_message.event))
                self.subscription_tasks.append(task)
            else:
                assert False, "unknown message type received"

            # Finish any outstanding subscription_tasks that are ready.
            for task in self.subscription_tasks:
                if task.done():
                    await task
                self.subscription_tasks.remove(task)

    async def _dispatch_message(self, **kwargs) -> asyncio.Future:
        message_id = next(self.message_id)
        assert message_id not in self.awaiting_response
        request = messages.ClientRequest.new_message(id=message_id, **kwargs)
        response_future = self.awaiting_response[message_id] = asyncio.Future()
        await self.websock.send(request.to_bytes())
        return response_future

    @staticmethod
    def _get_concrete_response(response: messages.ServerResponse):
        for name in ('ok', 'getFile', 'getMatchingFiles', 'listDirectory', 'watch', 'ping'):
            if response.which() == name:
                return getattr(response, name)
        raise NotImplementedError("unknown response type: {}".format(response.which()))

    def _handle_response_message(self, response: messages.ServerResponse):
        response_future = self.awaiting_response.pop(response.id)
        if response.which() == 'error':
            err = self.make_error(response.error)
            response_future.set_exception(err)
            return
        concrete = self._get_concrete_response(response)
        if not response_future.cancelled():
            response_future.set_result(concrete)

    async def _handle_subscription_message(self, event: messages.WatchedFilesChangedMessage):
        sid = event.subscriptionId
        if sid not in self.subscriptions:
            log.critical("received unknown subscription: {}", sid)
            return
        try:
            changes = {s.data: list(s.paths) for s in event.changes}
            cb = self.subscriptions[sid]
            await cb(changes)
        except Exception as e:
            log.critical("Handler for subscription id {} failed with exception:", sid)
            log.exception(e)

    @staticmethod
    def make_error(error: messages.ErrorResponse):
        exc_class = globals().get(error.name, UnknownErrorType)
        return exc_class(error.name, error.context)

    # ########## LowLevel Async ##########
    async def create_file_async(self, parent_path: str, name: str):
        return await self._dispatch_message(createFile=messages.CreateFileRequest.new_message(parentPath=parent_path,
                                                                                              name=name))

    async def create_formula_async(self, parent_path: str, name: str, inputs: {str: str}, formula: str):
        new_input = messages.CreateFormulaRequest.Input.new_message
        input_list = [new_input(name=k, path=v) for k, v in inputs.items()]
        return await self._dispatch_message(createFormula=messages.CreateFormulaRequest.new_message(
            parentPath=parent_path, name=name, inputs=input_list, formula=formula))

    async def create_directory_async(self, parent_path: str, name: str):
        return await self._dispatch_message(createDirectory=messages.CreateDirectoryRequest.new_message(
            parentPath=parent_path, name=name))

    async def remove_node_async(self, parent_path: str, name: str) -> asyncio.Future:
        return await self._dispatch_message(removeNode=messages.RemoveNodeRequest.new_message(parentPath=parent_path,
                                                                                              name=name))

    async def list_directory_async(self, path: str) -> asyncio.Future:
        return await self._dispatch_message(listDirectory=messages.ListDirectoryRequest.new_message(path=path))

    async def set_matching_files_async(self, glob: str, content: str) -> asyncio.Future:
        return await self._dispatch_message(setMatchingFiles=messages.SetMatchingFilesRequest.new_message(glob=glob,
                                                                                                          data=content))

    async def get_matching_files_async(self, glob: str) -> asyncio.Future:
        return await self._dispatch_message(getMatchingFiles=messages.GetMatchingFilesRequest.new_message(glob=glob))

    async def set_file_async(self, path: str, content: str) -> asyncio.Future:
        return await self._dispatch_message(setFile=messages.SetFileRequest.new_message(path=path, data=content))

    async def get_file_async(self, path: str) -> asyncio.Future:
        return await self._dispatch_message(getFile=messages.GetFileRequest.new_message(path=path))

    # Note that subscribe and unsubscribe do not have async varieties.
    # We do not guarantee message delivery order, so it is possible to
    # receive a subscription message before the subscription is
    # registered on this side of the channel.

    # ########## LowLevel Sync ##########
    async def create_file(self, parent_path: str, name: str):
        future = await self.create_file_async(parent_path, name)
        await future

    async def create_formula(self, parent_path: str, name: str, inputs: {str: str}, formula: str):
        future = await self.create_formula_async(parent_path, name, inputs, formula)
        await future

    async def create_directory(self, parent_path: str, name: str):
        future = await self.create_directory_async(parent_path, name)
        await future

    async def remove_node(self, parent_path: str, name: str):
        future = await self.remove_node_async(parent_path, name)
        await future

    async def list_directory(self, path: str) -> [str]:
        future = await self.list_directory_async(path)
        result = await future
        return list(result.children)

    async def set_file(self, path: str, content: str):
        future = await self.set_file_async(path, content)
        await future

    async def set_matching_files(self, glob: str, content: str):
        future = await self.set_matching_files_async(glob, content)
        await future

    async def get_file(self, path: str) -> str:
        future = await self.get_file_async(path)
        result = await future
        return result.data

    async def get_matching_files(self, glob: str) -> str:
        future = await self.get_matching_files_async(glob)
        result = await future
        return {x.path: x.data for x in result.data}

    async def watch_matching_files(self, glob: str, cb: callable) -> int:
        future = await self._dispatch_message(
            watchMatchingFiles=messages.WatchMatchingFilesRequest.new_message(glob=glob))
        result = await future
        self.subscriptions[result.subscriptionId] = cb
        return result.subscriptionId

    async def unwatch(self, sid: int):
        future = await self._dispatch_message(unwatch=messages.UnwatchRequest.new_message(subscriptionId=sid))
        await future
        del self.subscriptions[sid]
