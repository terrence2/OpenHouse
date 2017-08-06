# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from .tree import Tree


class Connection:
    """
    An async context manager to create and clean up a Tree connection.
    """
    def __init__(self, port: int):
        self.port = port
        self.connection = None

    @classmethod
    def from_args(cls, args):
        return cls(args.db_port)

    async def __aenter__(self):
        self.connection = await Tree.connect(self.port)
        return self.connection

    async def __aexit__(self, exc, *args):
        await self.connection.close()


async def make_connection(args):
    """
    A function to make a tree connection from args.
    """
    return await Tree.connect(args.db_port)
