#!/usr/bin/env python2
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from __future__ import print_function

from ouimeaux.environment import Environment
from ouimeaux.signals import devicefound, statechange, receiver, subscription


if __name__ == '__main__':

    @receiver(devicefound)
    def found(sender, **kwargs):
        print("Found device:", sender.name)

    @receiver(subscription)
    def subscription(sender, **kwargs):
        print("Subscription Result:", sender.name, kwargs['type'], kwargs['value'])

    try:
        env = Environment(with_cache=False, bind="0.0.0.0:54321")
        env.start()
        print("Discovering devices...")
        env.discover(10)
        print("Finished discovery.")

        motion = env.get_motion('WeMo Motion')
        print("list services: ", motion.list_services())
        #help(motion)
        #motion.explain()

        @receiver(statechange, sender=motion)
        def motion(sender, **kwargs):
            print("{} state is {state}".format(
                    sender.name, state="on" if kwargs.get('state') else "off"))

        env.wait()
    except (KeyboardInterrupt, SystemExit):
        print("Goodbye!")
        sys.exit(0)
