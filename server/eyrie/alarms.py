# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import llfuse

from eyrie.state import EyrieStateMachine


def _alarm_name(name, day):
    return 'alarm_{}_{}'.format(name, day)


def _trigger_alarm(state: EyrieStateMachine, name: str, day: str):
    if name == 'wakeup':
        state.change_state('auto:wakeup')
    elif name == 'sleep':
        state.change_state('auto:bedtime')


def populate_alarms(state: EyrieStateMachine):
    """
    Build alarm functions and put them on the global. This is separate from normal init
    because it has to be done very early in init, before initializing apscheduler.
    """
    for name in ['wakeup', 'sleep']:
        for day in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
            def make_alarm(bound_name, bound_day):
                def alarm():
                    with llfuse.lock:
                        _trigger_alarm(state, bound_name, bound_day)
                global_name = _alarm_name(bound_name, bound_day)
                alarm.__name__ = global_name
                globals()[global_name] = alarm
            make_alarm(name, day)

