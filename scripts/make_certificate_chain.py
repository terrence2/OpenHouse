#!/usr/bin/env python3
import os
import sys

sys.path.append(os.path.join(os.getcwd(), "scripts"))
from lib import call, change_directory
from lib.certificate_authority import\
    make_ca_directory_structure,\
    make_certificate_authority


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


def make_certificate(name: str, extension: str, key_size: int, expire: int,
                     key_security: str, x509_security: str):
    params = {
        'name': name,
        'extension': extension,
        'key_size': key_size,
        'expire': expire,
        'key_security': key_security,
        'x509_security': x509_security
    }
    with change_directory("intermediate"):
        call("openssl genrsa {key_security} -out private/{name}.key.pem {key_size}", **params)
        os.chmod("private/{name}.key.pem".format(**params), 0o400)
        call("""openssl req -config openssl.cnf -new {x509_security}
                                    -subj /C=US/ST=CA/L=SB/O=Me/OU=OpenHouse/CN={name}/
                                    -key private/{name}.key.pem
                                    -out csr/{name}.csr.pem""", **params)
    call("""openssl ca -config intermediate/openssl.cnf
                           -extensions {extension} -batch
                           -days {expire} -notext -md sha256
                           -in intermediate/csr/{name}.csr.pem
                           -out intermediate/certs/{name}.cert.pem""",
         **params)
    os.chmod("intermediate/certs/{name}.cert.pem".format(**params), 0o444)


def make_certificates():
    for server in SERVERS:
        make_certificate(server, 'server_cert', key_size=4096, expire=100*365,
                         key_security='', x509_security='-nodes')

    for client in CLIENTS:
        make_certificate(client, 'usr_cert', key_size=4096, expire=100*365,
                         key_security='', x509_security='-nodes')


def main():
    if not os.path.isfile("LICENSE"):
        print("This program expects to be run from the root OpenHouse checkout directory.")
        return 1

    make_ca_directory_structure()

    with change_directory("CA"):
        make_certificate_authority(key_size=8192, expire=100*365, key_security='', x509_security='-nodes')

    with change_directory("CA"):
        make_certificates()

    return 0


if __name__ == '__main__':
    import sys
    sys.exit(main())
