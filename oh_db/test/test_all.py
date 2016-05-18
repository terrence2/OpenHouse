#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import json
import pytest
import subprocess
import websockets
from contextlib import contextmanager
import db


#@pytest.yield_fixture(autouse=True)

@contextmanager
def run_server():
    target = './target/debug/oh_db'
    port = str(8899)
    proc = subprocess.Popen([target, '--address', 'localhost', '--port', port])
    try:
        yield
    finally:
        proc.terminate()
        proc.wait()



@pytest.mark.asyncio
async def test_create_sync():
    with run_server():
        async with db.Connection(("localhost", 8899)) as tree:
            for a in "abcd":
                await tree.create_child("/", a)
                for b in "efgh":
                    await tree.create_child("/{}".format(a), b)
                    for c in "ijkl":
                        await tree.create_child("/{}/{}".format(a, b), c)
                        for d in "mnop":
                            await tree.create_child("/{}/{}/{}".format(a, b, c), d)

            path = "/"
            assert "".join(sorted(await tree.list_children(path))) == "abcd"
            for a in await tree.list_children(path):
                a_path = path + a
                assert "".join(sorted(await tree.list_children(a_path))) == "efgh"
                for b in await tree.list_children(a_path):
                    b_path = a_path + "/" + b
                    assert "".join(sorted(await tree.list_children(b_path))) == "ijkl"
                    for c in await tree.list_children(b_path):
                        c_path = b_path + "/" + c
                        assert "".join(sorted(await tree.list_children(c_path))) == "mnop"
                        for d in await tree.list_children(c_path):
                            d_path = c_path + "/" + d
                            assert "".join(sorted(await tree.list_children(d_path))) == ""


@pytest.mark.asyncio
async def test_create_async():
    with run_server():
        async with db.Connection(("localhost", 8899)) as tree:
            futures = []
            children = "abcdefghijkl"
            for a in children:
                futures.append(tree.create_child_async("/", a))
            await asyncio.gather(*futures)

            future = await tree.list_children_async("/")
            result = await future
            assert "".join(sorted(result["children"])) == children


@pytest.mark.asyncio
async def test_create_errors():
    with run_server():
        async with db.Connection(("localhost", 8899)) as tree:
            await tree.create_child("/", "a")
            with pytest.raises(db.NodeAlreadyExists):
                await tree.create_child("/", "a")
            with pytest.raises(db.NoSuchNode):
                await tree.create_child("/b", "a")
            with pytest.raises(db.InvalidPathComponent):
                await tree.create_child("/", "a/b")
            with pytest.raises(db.MalformedPath):
                await tree.create_child("/../../usr/lib/", "libGL.so")


