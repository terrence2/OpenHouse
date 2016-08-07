# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import asyncio
import oh_shared.db as db
import pytest


@pytest.mark.asyncio
async def test_tree_async():
    with run_server():
        async with make_connection() as tree:
            futures = []
            children = "abcdefghijklmnopqrstuvwxyz"
            for i, a in enumerate(children):
                ty = db.NodeType.directory if i % 2 == 0 else db.NodeType.file
                futures.append(await tree.create_node_async(ty, "/", a))
                if i % 2 == 1:
                    futures.append(await tree.set_file_content_async("/" + a, a))
            await asyncio.gather(*futures)

            future = await tree.list_directory_async("/")
            result = await future
            assert "".join(sorted(result.children)) == children

            futures = []
            for i, a in enumerate(children):
                if i % 2 == 1:
                    futures.append(await tree.get_file_content_async("/" + a))
            results = await asyncio.gather(*futures)
            assert "".join(sorted([rv.data for rv in results])) == children[1::2]

            futures = []
            vowels = "aeiou"
            for a in vowels:
                futures.append(await tree.remove_node_async("/", a))
            await asyncio.gather(*futures)

            future = await tree.list_directory_async("/")
            result = await future
            assert "".join(sorted(result.children)) == \
                   "bcdfghjklmnpqrstvwxyz"


@pytest.mark.asyncio
async def test_subscribe_same_client_layout():
    """
    Ensure that subscriptions work and that we can:
      * make multiple subscriptions to the same path on a single client.
      * touch a subpath without being notified in the parent
      * remove one subscription of multiple
    """
    with run_server():
        async with make_connection() as tree:
            count1 = 0
            notify1 = asyncio.Future()

            async def on_child_changed1(paths: str, event: db.EventKind, name: str):
                assert len(paths) == 1
                assert paths[0] == "/"
                assert name == "a"
                assert event == db.EventKind.created
                nonlocal count1, notify1
                count1 += 1
                notify1.set_result(...)

            count2 = 0
            notify2 = asyncio.Future()

            async def on_child_changed2(paths: str, event: db.EventKind, name: str):
                assert len(paths) == 1
                assert paths[0] == "/"
                assert name == "a"
                assert event == db.EventKind.created or event == db.EventKind.removed
                nonlocal count2, notify2
                count2 += 1
                notify2.set_result(...)

            subid1 = await tree.subscribe("/", on_child_changed1)
            subid2 = await tree.subscribe("/", on_child_changed2)

            # Check that we get messages when we create the first child, but not the grandchild.
            await tree.create_directory("/", "a")
            await tree.create_directory("/a", "b")
            await tree.remove_node("/a", "b")
            await asyncio.gather(notify1, notify2)
            assert count1 == 1
            assert count2 == 1

            # Reset notifications; unsubscribe #1, then check that we only get the notice on 2.
            notify1 = asyncio.Future()
            notify2 = asyncio.Future()
            await tree.unsubscribe(subid1)
            await tree.remove_node("/", "a")
            await asyncio.sleep(0.1)  # we don't expect a response from 1, but give it some time to be more sure.
            await notify2
            assert count1 == 1
            assert count2 == 2


@pytest.mark.asyncio
async def test_subscribe_same_client_data():
    """
    Ensure that subscriptions work and that we can:
      * make multiple subscriptions to the same path on a single client.
      * touch a subpath without being notified in the parent
      * remove one subscription of multiple
    """
    with run_server():
        async with make_connection() as tree:
            count1 = 0
            notify1 = asyncio.Future()

            async def on_child_changed1(paths: str, event: db.EventKind, context: str):
                assert len(paths) == 1
                assert paths[0] == "/a"
                assert context == "foo"
                assert event == db.EventKind.changed
                nonlocal count1, notify1
                count1 += 1
                notify1.set_result(...)

            count2 = 0
            notify2 = asyncio.Future()

            async def on_child_changed2(paths: str, event: db.EventKind, context: str):
                assert len(paths) == 1
                assert paths[0] == "/a"
                assert context == "foo"
                assert event == db.EventKind.changed or event == db.EventKind.removed
                nonlocal count2, notify2
                count2 += 1
                notify2.set_result(...)

            # Create and subscribe to data node.
            await tree.create_node(db.NodeType.file, "/", "a")
            await tree.create_node(db.NodeType.file, "/", "b")
            subid1 = await tree.subscribe("/a", on_child_changed1)
            subid2 = await tree.subscribe("/a", on_child_changed2)

            # Check that we get messages when we change the data, but not when we set siblings, or query it.
            await tree.set_file_content("/a", "foo")
            await tree.set_file_content("/b", "foo")
            rv = await tree.get_file_content("/a")
            assert rv == "foo"
            await asyncio.gather(notify1, notify2)
            assert count1 == 1
            assert count2 == 1

            # Reset notifications; unsubscribe #1, then check that we only get the notice on 2.
            notify1 = asyncio.Future()
            notify2 = asyncio.Future()
            await tree.unsubscribe(subid1)
            await tree.set_file_content("/a", "foo")
            await asyncio.sleep(0.1)  # we don't expect a response from 1, but give it some time to be more sure.
            await notify2
            assert count1 == 1
            assert count2 == 2


@pytest.mark.asyncio
async def test_subscribe_multiple_clients():
    """
    Ensure that causing an event on one client reports that event on a different client.
    """
    with run_server():
        async with make_connection() as treeA:
            async with make_connection() as treeB:
                count = 0
                notify = asyncio.Future()

                async def on_child_changed1(paths: str, event: db.EventKind, context: str):
                    assert len(paths) == 1
                    assert paths[0] == "/"
                    assert context == "a"
                    nonlocal count, notify
                    count += 1
                    if event == db.EventKind.removed:
                        notify.set_result(...)

                await treeA.subscribe("/", on_child_changed1)

                await treeB.create_directory("/", "a")
                await treeB.remove_node("/", "a")

                await notify

                assert count == 2



