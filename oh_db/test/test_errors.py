# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import oh_shared.db as db
import pytest


NodeTypes = (db.NodeType.directory, db.NodeType.file)


@pytest.mark.asyncio
async def test_create_errors():
    with run_server():
        async with make_connection() as tree:
            await tree.create_node(db.NodeType.directory, "/", "dir")
            await tree.create_node(db.NodeType.file, "/", "file")
            for ty in NodeTypes:
                with pytest.raises(db.InvalidCharacter):
                    await tree.create_node(ty, "/", "dir/b")
                with pytest.raises(db.Dotfile):
                    await tree.create_node(ty, "/../../usr/lib/", "libGL.so")
                with pytest.raises(db.EmptyComponent):
                    await tree.create_node(ty, "//", "libGL.so")
                with pytest.raises(db.EmptyComponent):
                    await tree.create_node(ty, "//foo", "libGL.so")
                with pytest.raises(db.EmptyComponent):
                    await tree.create_node(ty, "/foo/", "libGL.so")
                with pytest.raises(db.EmptyComponent):
                    await tree.create_node(ty, "/foo//bar", "libGL.so")
                with pytest.raises(db.NoSuchNode):
                    await tree.create_node(ty, "/b", "a")
                with pytest.raises(db.NodeAlreadyExists):
                    await tree.create_node(ty, "/", "dir")
                with pytest.raises(db.NotDirectory):
                    await tree.create_node(ty, "/file", 'foo')


@pytest.mark.asyncio
async def test_remove_errors():
    with run_server():
        async with make_connection() as tree:
            with pytest.raises(db.InvalidCharacter):
                await tree.remove_node("/", "a/b")
            with pytest.raises(db.Dotfile):
                await tree.remove_node("/../../usr/lib/", "libGL.so")
            with pytest.raises(db.NoSuchNode):
                await tree.remove_node("/", "a")

            await tree.create_node(db.NodeType.directory, "/", "a")
            await tree.create_node(db.NodeType.directory, "/a", "b")
            with pytest.raises(db.DirectoryNotEmpty):
                await tree.remove_node("/", "a")
            await tree.remove_node("/a", "b")


@pytest.mark.asyncio
async def test_data_errors():
    with run_server():
        async with make_connection() as tree:
            with pytest.raises(db.NotFile):
                await tree.set_file_content("/", "")
            with pytest.raises(db.NotFile):
                await tree.get_file_content("/")
            with pytest.raises(db.Dotfile):
                await tree.set_file_content("/.", "")
            with pytest.raises(db.Dotfile):
                await tree.get_file_content("/.")
            with pytest.raises(db.NonAbsolutePath):
                await tree.set_file_content("a/b", "")
            with pytest.raises(db.NonAbsolutePath):
                await tree.get_file_content("a/b")

            await tree.create_file("/", "a")
            with pytest.raises(db.NotDirectory):
                await tree.set_file_content("/a/b", "")


@pytest.mark.asyncio
async def test_subscribe_errors():
    with run_server():
        async with make_connection() as tree:
            async def target(**_):
                pass

            with pytest.raises(db.Dotfile):
                await tree.subscribe("/../../usr/lib/libGL.so", target)


@pytest.mark.asyncio
async def test_unsubscribe_errors():
    with run_server():
        async with make_connection() as tree:
            with pytest.raises(db.NoSuchSubscription):
                await tree.unsubscribe(10)


