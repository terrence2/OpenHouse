# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from apscheduler.scheduler import Scheduler

import llfuse

from eyrie.state import EyrieStateMachine

from mcp.filesystem import FileSystem, File, Directory


def _alarm_name(name: str, day: str) -> str:
    return 'alarm_{}_{}'.format(name, day)


def _get_alarm(name: str, day: str) -> callable:
    return globals()[_alarm_name(name, day)]


def _find_scheduler_job(scheduler: Scheduler, alarm_func: callable):
    jobs = scheduler.get_jobs()
    for job in jobs:
        if job.func == alarm_func:
            return job
    return None


def _map_filesystem_to_scheduler_day(day: str) -> str:
    return {'monday': 'mon',
            'tuesday': 'tue',
            'wednesday': 'wed',
            'thursday': 'thu',
            'friday': 'fri',
            'saturday': 'sat',
            'sunday': 'sun'}[day]


def populate_alarms_and_bind_to_state(state: EyrieStateMachine):
    """
    Build alarm functions and put them on the global. This is separate from normal init
    because it has to be done very early in init, before initializing apscheduler.
    """
    for name in ['wakeup', 'sleep']:
        for day in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
            def make_alarm(bound_name, bound_day):
                def alarm():
                    with llfuse.lock:
                        if name == 'wakeup':
                            state.change_state('auto:wakeup')
                        elif name == 'sleep':
                            state.change_state('auto:bedtime')
                global_name = _alarm_name(bound_name, bound_day)
                alarm.__name__ = global_name
                globals()[global_name] = alarm
            make_alarm(name, day)


def bind_alarms_to_filesystem(scheduler: Scheduler, filesystem: FileSystem):
    """
    Binds the callable alarm functions we made earlier into the system. Apscheduler should
    have picked up any existing schedules automatically.
    """
    def alarms_help() -> str:
        return "Values are: 'off' or |24-hour-time|.\nExample: '7:30' or '16:42'.\n"

    # Now that we've initialized the rest of the system, install our alarms on the filesystem.
    alarms_dir = filesystem.root().add_entry("alarms", Directory())
    alarms_dir.add_entry("help", File(alarms_help, None))
    for name in ['wakeup', 'sleep']:
        alarm_dir = alarms_dir.add_entry(name, Directory())
        for day in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
            def make_alarm_file(bound_name, bound_day):
                """Closure to capture the right values for loop vars name and day."""
                def read_alarm() -> str:
                    alarm_func = _get_alarm(bound_name, bound_day)
                    job = _find_scheduler_job(scheduler, alarm_func)
                    value = 'off'
                    if job is not None:
                        value = str(job.trigger)
                    return "Alarm {} for {}: {}\n".format(bound_name, bound_day, value)

                def write_alarm(data: str):
                    data = data.strip()
                    alarm_func = _get_alarm(bound_name, bound_day)
                    existing_job = _find_scheduler_job(scheduler, alarm_func)

                    if data == 'off':
                        if existing_job:
                            scheduler.unschedule_job(existing_job)
                        return

                    hour, _, minute = data.strip().partition(':')
                    hour = min(23, max(0, int(hour)))
                    minute = min(59, max(0, int(minute)))
                    day_of_week = _map_filesystem_to_scheduler_day(bound_day)
                    if existing_job:
                        scheduler.unschedule_job(existing_job)
                    scheduler.add_cron_job(alarm_func, day_of_week=day_of_week, hour=hour, minute=minute)

                return File(read_alarm, write_alarm)
            alarm_dir.add_entry(day, make_alarm_file(name, day))
