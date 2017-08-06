#!/usr/bin/env python3
import argparse
import os
from lib.certificate_authority import make_client_certificate


def main():
    if not os.path.isfile("LICENSE"):
        print("This program expects to be run from the root OpenHouse checkout directory.")
        return 1

    parser = argparse.ArgumentParser(description="Create a new certificate under CA")
    parser.add_argument("-n", "--name", required=True, type=str, help="The name of the new certificate")
    args = parser.parse_args()

    make_client_certificate(args.name, key_size=4096, expire=100*365, key_security='', x509_security='-nodes')

if __name__ == '__main__':
    main()
