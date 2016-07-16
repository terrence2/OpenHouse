# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from collections import namedtuple
from contextlib import contextmanager
from oh_shared.db import Connection
import os
import subprocess


ServerConfig = namedtuple('ServerConfig', ['target', 'address', 'port', 'ca_chain', 'certificate', 'private_key'])
ClientConfig = namedtuple('ClientConfig', ['ca_chain', 'certificate', 'private_key'])

server_config = ServerConfig("./target/debug/oh_db", "localhost", "8899",
                             "../CA/intermediate/certs/chain.cert.pem",
                             "../CA/intermediate/certs/oh_db.cert.pem",
                             "../CA/intermediate/private/oh_db.key.pem")


client_config = ClientConfig("../CA/intermediate/certs/chain.cert.pem",
                             "../CA/intermediate/certs/oh_db_test.cert.pem",
                             "../CA/intermediate/private/oh_db_test.key.pem")


@contextmanager
def run_server():
    env = os.environ.copy()
    env['RUST_BACKTRACE'] = str(1)
    proc = subprocess.Popen([server_config.target,
                             '--address', server_config.address,
                             '--port', server_config.port,
                             '--ca-chain', server_config.ca_chain,
                             '--certificate', server_config.certificate,
                             '--private-key', server_config.private_key], env=env)
    try:
        yield
    finally:
        proc.terminate()
        proc.wait()


def make_connection():
    return Connection((server_config.address, server_config.port),
                       client_config.ca_chain,
                       client_config.certificate,
                       client_config.private_key)


