# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from .tree import Tree


class Connection:
    """
    An async context manager to create and clean up a Tree connection.
    """
    def __init__(self, address: (str, int), ca_cert_chain: str, cert_chain: str, key_file: str):
        self.address = address
        self.ca_cert_chain = ca_cert_chain
        self.cert_chain = cert_chain
        self.key_file = key_file
        self.connection = None

    @classmethod
    def from_args(cls, args):
        return cls((args.home_address, args.home_port), args.ca_chain, args.certificate, args.private_key)

    async def __aenter__(self):
        self.connection = await Tree.connect(self.address, self.ca_cert_chain,
                                             self.cert_chain, self.key_file)
        return self.connection

    async def __aexit__(self, exc, *args):
        await self.connection.close()
