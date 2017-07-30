#!/usr/bin/env python3
import os
import sys

sys.path.append(os.path.join(os.getcwd(), "scripts"))
from lib import call, change_directory
from lib.certificate_authority import\
    make_certificate_authority,\
    make_server_certificate,\
    make_client_certificate


SERVERS = [
    "oh_db",
    "oh_tree",
    "oh_home"
]

CLIENTS = [
    "oh_button",
    "oh_cli",
    "oh_color",
    "oh_db_test",
    "oh_fs",
    "oh_hue",
    "oh_populate",
    "oh_rest",
    "oh_supervisor",
]

def main():
    if not os.path.isfile("LICENSE"):
        print("This program expects to be run from the root OpenHouse checkout directory.")
        return 1

    make_certificate_authority(key_size=8192, expire=100*365, key_security='', x509_security='-nodes')

    for server in SERVERS:
        make_server_certificate(server, key_size=4096, expire=100*365, key_security='', x509_security='-nodes')

    for client in CLIENTS:
        make_client_certificate(client, key_size=4096, expire=100*365, key_security='', x509_security='-nodes')

    return 0


if __name__ == '__main__':
    import sys
    sys.exit(main())
