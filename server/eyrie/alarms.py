# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import llfuse


def _alarm_name(name, day):
    return 'alarm_{}_{}'.format(name, day)


def populate_alarms(target_):
    """
    Build alarm functions and put them on the global. This is separate from normal init
    because it has to be done very early in init, before initializing apscheduler.
    """
    for name_ in ['wakeup', 'sleep']:
        for day_ in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
            def make_alarm(target, name, day):
                def alarm():
                    with llfuse.lock:
                        target.trigger_alarm(name, day)
                global_name = _alarm_name(name, day)
                alarm.__name__ = global_name
                globals()[global_name] = alarm
            make_alarm(target_, name_, day_)

