# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import asyncio
import oh_shared.db as db
import pytest


@pytest.mark.asyncio
async def test_subscribe_same_client_layout():
    """
    Ensure that subscriptions work on a directory and that we can:
      * make multiple subscriptions to the same directory on a single client.
      * touch a subpath without being notified in the parent
      * remove one subscription of multiple
    """
    with run_server():
        async with make_connection() as tree:
            count1 = 0
            notify1 = asyncio.Future()

            async def on_child_changed1(paths: [str], event: db.EventKind, name: str):
                assert len(paths) == 1
                assert paths[0] == "/"
                assert name == "a"
                assert event == db.EventKind.created
                nonlocal count1, notify1
                count1 += 1
                notify1.set_result(...)

            count2 = 0
            notify2 = asyncio.Future()

            async def on_child_changed2(paths: [str], event: db.EventKind, name: str):
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
    Ensure that subscriptions to data work and that we can:
      * make multiple subscriptions to the same path on a single client.
      * touch a subpath without being notified in the parent
      * remove one subscription of multiple
    """
    with run_server():
        async with make_connection() as tree:
            count1 = 0
            notify1 = asyncio.Future()

            async def on_child_changed1(paths: [str], event: db.EventKind, context: str):
                assert len(paths) == 1
                assert paths[0] == "/a"
                assert context == "foo"
                assert event == db.EventKind.changed
                nonlocal count1, notify1
                count1 += 1
                notify1.set_result(...)

            count2 = 0
            notify2 = asyncio.Future()

            async def on_child_changed2(paths: [str], event: db.EventKind, context: str):
                assert len(paths) == 1
                assert paths[0] == "/a"
                assert context == "foo"
                assert event == db.EventKind.changed or event == db.EventKind.removed
                nonlocal count2, notify2
                count2 += 1
                notify2.set_result(...)

            # Create and subscribe to data node.
            await tree.create_file("/", "a")
            await tree.create_file("/", "b")
            subid1 = await tree.subscribe("/a", on_child_changed1)
            subid2 = await tree.subscribe("/a", on_child_changed2)

            # Check that we get messages when we change the data, but not when we set siblings, or query it.
            await tree.set_file("/a", "foo")
            await tree.set_file("/b", "foo")
            data = await tree.get_file("/a")
            assert data == "foo"
            await asyncio.gather(notify1, notify2)
            assert count1 == 1
            assert count2 == 1

            # Reset notifications; unsubscribe #1, then check that we only get the notice on 2.
            notify1 = asyncio.Future()
            notify2 = asyncio.Future()
            await tree.unsubscribe(subid1)
            await tree.set_file("/a", "foo")
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

                async def on_child_changed1(paths: [str], event: db.EventKind, context: str):
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


@pytest.mark.asyncio
async def test_subscribe_glob_basic():
    """
    Ensure that subscribing to a glob works properly.
    """
    with run_server():
        async with make_connection() as tree:
            expect = None
            count = 0
            async def on_foo_changed(paths: [str], event: db.EventKind, context: str):
                nonlocal count
                assert expect is not None
                assert expect == (count, list(paths), event, context)
                count += 1

            sid = await tree.subscribe("/?-foo", on_foo_changed)

            # We should be able to create a new path and have the glob pick it up.
            await tree.create_directory("/", "a-foo")

            # Check with creating and removing a directory.
            expect = (0, ["/a-foo"], db.EventKind.created, "test")
            await tree.create_directory("/a-foo", "test")
            expect = (1, ["/a-foo"], db.EventKind.removed, "test")
            await tree.remove_node("/a-foo", "test")
            expect = None

            # We should not get a notification on removing the watched node.
            await tree.remove_node("/", "a-foo")

            # We should not get a notification for inserting into a non-matching directory.
            await tree.create_directory("/", "a-bar")
            await tree.create_directory("/a-bar", "test")
            await tree.remove_node("/a-bar", "test")
            await tree.remove_node("/", "a-bar")

            # We should not get a notice when creating a file of the matched name.
            await tree.create_file("/", "a-foo")

            expect = (2, ["/a-foo"], db.EventKind.changed, "hello")
            await tree.set_file("/a-foo", "hello")
            expect = (3, ["/a-foo"], db.EventKind.removed, "???")
            await tree.remove_node("/", "a-foo")
            expect = None


@pytest.mark.asyncio
async def test_subscribe_glob_filter():
    """
    Ensure that globs only match matching files.
    """
    with run_server():
        async with make_connection() as tree:
            expect = None
            count = 0
            async def on_changed(paths: [str], event: db.EventKind, context: str):
                nonlocal count
                assert expect is not None
                assert expect == (count, sorted(list(paths)), event, context)
                count += 1

            for name in ['a', 'b', 'c', 'aa']:
                await tree.create_file("/", name)
            await tree.subscribe("/?", on_changed)

            expect = (0, ["/a"], db.EventKind.changed, "foo")
            await tree.set_file("/a", "foo")
            await tree.set_file("/aa", "foo")
            expect = (1, ["/b"], db.EventKind.changed, "bar")
            await tree.set_file("/b", "bar")
            await tree.set_file("/aa", "bar")
            expect = (2, ["/c"], db.EventKind.changed, "baz")
            await tree.set_file("/c", "baz")
            await tree.set_file("/aa", "baz")


@pytest.mark.asyncio
async def test_subscribe_glob_multi():
    """
    Ensure that globs return all matching path events in a single message.
    """
    with run_server():
        async with make_connection() as tree:
            expect = None
            count = 0
            async def on_changed(paths: [str], event: db.EventKind, context: str):
                nonlocal count
                assert expect is not None
                assert expect == (count, sorted(list(paths)), event, context)
                count += 1

            for a in ['a', 'b', 'c']:
                await tree.create_directory("/", a)
                for b in ['a', 'b', 'c']:
                    await tree.create_directory("/{}".format(a), b)
                    for c in ['foo', 'bar', 'baz']:
                        await tree.create_file("/{}/{}".format(a, b), c)
            await tree.subscribe("/a/**/foo", on_changed)

            expect = (0, ["/a/a/foo"], db.EventKind.changed, "a")
            await tree.set_file("/a/a/foo", "a")
            expect = (1, ["/a/a/foo"], db.EventKind.changed, "b")
            await tree.set_matching_files("/a/a/*", "b")
            await tree.set_matching_files("/**/bar", "c")
            expect = (2, ["/a/a/foo", "/a/b/foo", "/a/c/foo"], db.EventKind.changed, "c")
            await tree.set_matching_files("/**/foo", "c")

