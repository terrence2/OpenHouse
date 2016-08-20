# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import asyncio
import pytest


@pytest.mark.asyncio
async def test_tree_sync():
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
async def test_data():
    with run_server():
        async with make_connection() as tree:
            await tree.create_file("/", "a")
            await tree.set_file("/a", "flinfniffle")
            data = await tree.get_file("/a")
            assert data == "flinfniffle"
            await tree.remove_node("/", "a")
