# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import oh_shared.db as db
import pytest


NodeTypes = (db.NodeType.directory, db.NodeType.file)

InvalidNames = {
    db.Dotfile: (".", "..", ".foo"),
}
EmptyComponentPaths = {
    db.EmptyComponent: ("//", "/foo/", "/foo//bar"),
}
InvalidChars = {
    db.InvalidCharacter: "/\\:,![]{}",
    db.InvalidControlCharacter: "\n\v\t\r",
    db.InvalidWhitespaceCharacter: " \u00A0",
    db.InvalidGlobCharacter: "?*",
}

@pytest.mark.asyncio
async def test_create_errors():
    with run_server():
        async with make_connection() as tree:
            await tree.create_node(db.NodeType.directory, "/", "dir")
            await tree.create_node(db.NodeType.file, "/", "file")

            for path in ("/", "/dir"):
                for ty in NodeTypes:
                    for exc_type, chars in InvalidChars.items():
                        for c in chars:
                            with pytest.raises(exc_type):
                                await tree.create_node(ty, path, "a" + c + "b")
                            if c != '/':
                                with pytest.raises(exc_type):
                                    await tree.create_node(ty, "/a" + c + "b", "foo")
                    for exc_type, names in InvalidNames.items():
                        for name in names:
                            with pytest.raises(exc_type):
                                await tree.create_node(ty, path, name)
                            with pytest.raises(exc_type):
                                await tree.create_node(ty, "/" + name, "foo")
                    for exc_type, names in EmptyComponentPaths.items():
                        for name in names:
                            with pytest.raises(exc_type):
                                await tree.create_node(ty, name, "foo")

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
            for exc_type, chars in InvalidChars.items():
                for c in chars:
                    with pytest.raises(exc_type):
                        await tree.remove_node("/", "a" + c + "b")
                    if c != '/':
                        with pytest.raises(exc_type):
                            await tree.remove_node("/a" + c + "b", "foo")
            for exc_type, names in InvalidNames.items():
                for name in names:
                    with pytest.raises(exc_type):
                        await tree.remove_node("/", name)
                    with pytest.raises(exc_type):
                        await tree.remove_node("/" + name, "foo")
            for exc_type, names in EmptyComponentPaths.items():
                for name in names:
                    with pytest.raises(exc_type):
                        await tree.remove_node(name, "foo")

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
            with pytest.raises(db.Dotfile):
                await tree.set_file("/.", "")
            with pytest.raises(db.Dotfile):
                await tree.get_file("/.")
            with pytest.raises(db.NonAbsolutePath):
                await tree.set_file("a/b", "")
            with pytest.raises(db.NonAbsolutePath):
                await tree.get_file("a/b")


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


