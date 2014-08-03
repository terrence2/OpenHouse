# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.state import EyrieStateMachine

from mcp.cronish import Cronish
from mcp.filesystem import FileSystem, File, Directory, StaticFile


def _alarm_name(name: str, day: str) -> str:
    return 'alarm_{}_{}'.format(name, day)


def _map_filesystem_to_scheduler_day(day: str) -> int:
    return {'monday': 0,
            'tuesday': 1,
            'wednesday': 2,
            'thursday': 3,
            'friday': 4,
            'saturday': 5,
            'sunday': 6}[day]


def _parse_alarm_string(data: str, day: str) -> (int, int, int):
    data = data.strip()
    dow = _map_filesystem_to_scheduler_day(day)
    hour, _, minute = data.strip().partition(':')
    if minute.endswith('+'):
        minute = minute.rstrip('+')
        dow = (dow + 1) % 7
    hour = min(23, max(0, int(hour)))
    minute = min(59, max(0, int(minute)))
    return dow, hour, minute


def bind_alarms_to_state(cronish: Cronish, state: EyrieStateMachine):
    def wakeup():
        state.change_state('auto:wakeup')

    def sleep():
        state.change_state('auto:bedtime')

    for day in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
        cronish.register_task('alarm_wakeup_{}'.format(day), wakeup)
        cronish.register_task('alarm_sleep_{}'.format(day), sleep)


def bind_alarms_to_filesystem(cronish: Cronish, filesystem: FileSystem):
    alarms_dir = filesystem.root().add_subdir("alarms", Directory())

    help = "Values are: 'off' or '[H]H:MM[+]'.\nExample: '7:30', '16:42', or '2:00+' for 2AM tomorrow.\n"
    alarms_dir.add_file("help", StaticFile(help))

    for name in ['wakeup', 'sleep']:
        alarm_dir = alarms_dir.add_subdir(name, Directory())
        for day in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
            def make_alarm_file(bound_name, bound_day):
                alarm_name = _alarm_name(bound_name, bound_day)

                def read_alarm() -> str:
                    return str(cronish.get_task(alarm_name)) + '\n'

                def write_alarm(data: str):
                    dow, hour, minute = _parse_alarm_string(data, bound_day)
                    cronish.update_task_time(alarm_name, days_of_week={dow}, hours={hour}, minutes={minute})

                return File(read_alarm, write_alarm)
            alarm_dir.add_file(day, make_alarm_file(name, day))

