# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import asyncio
import pytest


@pytest.mark.asyncio
async def test_initial_tree():
    """
    Ensure that the tree starts empty.
    """
    with run_server():
        async with make_connection() as tree:
            assert await tree.list_directory("/") == []


@pytest.mark.asyncio
async def test_data():
    """
    Check that basic create/get/set/remove works as expected.
    """
    with run_server():
        async with make_connection() as tree:
            await tree.create_file("/", "a")
            await tree.set_file("/a", "flinfniffle")
            data = await tree.get_file("/a")
            assert data == "flinfniffle"
            await tree.remove_node("/", "a")


@pytest.mark.asyncio
async def test_set_glob_basic():
    """
    Ensure that globs only match matching files.
    """
    with run_server():
        async with make_connection() as tree:
            for name in "abcd":
                await tree.create_file("/", name)
            await tree.set_matching_files("/*", "hello")
            data = await tree.get_matching_files("/*")
            for name in "abcd":
                assert data["/" + name] == "hello"


@pytest.mark.asyncio
async def test_tree_sync():
    """
    Create a large tree using the sync API and verify the content and layout.
    """
    with run_server():
        async with make_connection() as tree:
            for a in "abcd":
                await tree.create_directory("/", a)
                for b in "efgh":
                    await tree.create_directory("/{}".format(a), b)
                    for c in "ijkl":
                        await tree.create_directory("/{}/{}".format(a, b), c)
                        for d in "mnop":
                            await tree.create_directory("/{}/{}/{}".format(a, b, c), d)

            path = "/"
            assert "".join(sorted(await tree.list_directory(path))) == "abcd"
            for a in await tree.list_directory(path):
                a_path = path + a
                assert "".join(sorted(await tree.list_directory(a_path))) == "efgh"
                for b in await tree.list_directory(a_path):
                    b_path = a_path + "/" + b
                    assert "".join(sorted(await tree.list_directory(b_path))) == "ijkl"
                    for c in await tree.list_directory(b_path):
                        c_path = b_path + "/" + c
                        assert "".join(sorted(await tree.list_directory(c_path))) == "mnop"
                        for d in await tree.list_directory(c_path):
                            d_path = c_path + "/" + d
                            assert "".join(sorted(await tree.list_directory(d_path))) == ""

            path = "/"
            for a in await tree.list_directory(path):
                for b in await tree.list_directory("/{}".format(a)):
                    for c in await tree.list_directory("/{}/{}".format(a, b)):
                        for d in await tree.list_directory("/{}/{}/{}".format(a, b, c)):
                            await tree.remove_node("/{}/{}/{}".format(a, b, c), d)
                        await tree.remove_node("/{}/{}".format(a, b), c)
                    await tree.remove_node("/{}".format(a), b)
                await tree.remove_node("/", a)


@pytest.mark.asyncio
async def test_tree_async():
    """
    Do a large amount of work using async calls.
    """
    with run_server():
        async with make_connection() as tree:
            futures = []
            children = "abcdefghijklmnopqrstuvwxyz"
            for i, a in enumerate(children):
                if i % 2 == 0:
                    futures.append(await tree.create_directory_async("/", a))
                else:
                    # Note that this is safe because we're using TCP under the hood. If we ever do anything non-serial,
                    # this test will need to change pretty dramatically.
                    futures.append(await tree.create_file_async("/", a))
                    futures.append(await tree.set_file_async("/" + a, a))
            await asyncio.gather(*futures)

            future = await tree.list_directory_async("/")
            result = await future
            assert "".join(sorted(result.children)) == children

            futures = []
            for i, a in enumerate(children):
                if i % 2 == 1:
                    futures.append(await tree.get_file_async("/" + a))
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


