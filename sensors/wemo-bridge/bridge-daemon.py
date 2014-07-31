#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import os
import sys
import subprocess

if __name__ != '__main__':
    print("This module must be run directly.\n")
    raise ImportError()

# Run the bridge script. When the watchdog kills it, restart it.
want_exit = False
while not want_exit:
    try:
        bridge_executable = os.path.realpath(os.path.join(os.getcwd(), 'bridge.py'))
        print("Running {} {}".format(bridge_executable, sys.argv[1:]))
        return_code = subprocess.call([bridge_executable] + sys.argv[1:])
        print("Subprocess exited with code: {}".format(return_code))
        want_exit = (return_code == 0)
    except KeyboardInterrupt:
        want_exit = True

print("Controller process exiting.")
