# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import json
import logging
import os
import os.path
import select

from datetime import datetime, time, timedelta
from threading import Thread


log = logging.getLogger('cronish')


class _Task:
    def _empty_runnable(self):
        pass

    def __init__(self, name: str):
        self.name = name
        self.callback_ = self._empty_runnable

        self.days_of_week = set()
        self.hours = set()
        self.minutes = set()

    def set_callback(self, callback: callable):
        self.callback_ = callback

    def set_time(self, days_of_week: {int}, hours: {int}, minutes: {int}):
        self.days_of_week = days_of_week
        self.hours = hours
        self.minutes = minutes

    def should_run_at(self, t: datetime) -> bool:
        return (t.weekday() in self.days_of_week and
                t.hour in self.hours and
                t.minute in self.minutes)

    def run(self):
        log.info("running task '{}'".format(self.name))
        return self.callback_()

    def __str__(self):
        return "Task(name={0.name},dow={0.days_of_week},hours={0.hours},minutes={0.minutes},set={1})".format(self,
               self.callback_ != self._empty_runnable)


class Cronish(Thread):
    """
    A cronish thread: unlike other alternatives, this package makes a ton of assumptions about usage in order to provide
    a simpler interface and more features.

    - Each task has a name and can only exist once in the system.
    - Each task will run 0 or 1 times in the 24 hour period between 0000 and 2359 local.
    - A task may be unscheduled, in which case it exists but is not run.
    - A task may be unmapped (to a suitable run function), in which case it will not be run at the set time.
    - There is a task database and scheduled times (but not run functions) are serialzed.

    This seems like an absurd list, but makes a certain use case much easier.

    Typical usage looks like:
    1) Create a Cronish instance. This will load any previously created tasks from disk with their scheduled time.
    2) As part of init, register all of your program's tasks. They may already exist, but will not yet have a runnable.
       This process will make the tasks runnable.
    3) (optional) Run Cronish.cull to strip any tasks without a runnable. This handles the case where the program
       has removed some tasks in a more recent version. Holding old tasks is only potentially dangerous with name
       re-use, but it is also overhead for serialization, etc.
    4) At runtime or during startup, create, remove, query, or change the schedule of tasks.
    5) During shutdown, call Cronish.save to serialize the task list.
    """

    def __init__(self, database_path: str, lock):
        super().__init__()
        self.daemon = True

        # Lock to take when running tasks.
        self.lock_ = lock

        # Set to True when we want to exit.
        self.want_exit_ = False

        # For unblocking us in the middle of a sleep.
        self.read_fd_, self.write_fd_ = os.pipe()

        # Load any existing tasks for disk.
        self.database_filename = os.path.join(database_path, 'crontab.json')
        self.tasks_ = self._load_tasks(self.database_filename)

    @staticmethod
    def _load_tasks(filename: str) -> {str: _Task}:
        try:
            with open(filename, 'r') as fp:
                data = json.load(fp)
        except FileNotFoundError:
            return {}
        except ValueError:
            return {}

        tasks = {}
        for key, value in data.items():
            tasks[key] = _Task(key)
            dow = set(value['days_of_week'])
            hours = set(value['hours'])
            minutes = set(value['minutes'])
            tasks[key].set_time(days_of_week=dow, hours=hours, minutes=minutes)
        return tasks

    @staticmethod
    def _save_tasks(filename: str, tasks: {str: _Task}):
        obj = {}
        for name, task in tasks.items():
            obj[name] = {
                'days_of_week': list(task.days_of_week),
                'hours': list(task.hours),
                'minutes': list(task.minutes)
            }
        with open(filename, 'w') as fp:
            json.dump(obj, fp)

    def register_task(self, name: str, callback: callable):
        if name not in self.tasks_:
            self.tasks_[name] = _Task(name)
        self.tasks_[name].set_callback(callback)
        log.info("registered task '{}'".format(name))

    def update_task_time(self, name: str, days_of_week: {int}, hours: {int}, minutes: {int}):
        self.tasks_[name].set_time(days_of_week, hours, minutes)
        log.info("Updated task '{}' to {}".format(name, str(self.tasks_[name])))
        self._save_tasks(self.database_filename, self.tasks_)

    def get_task(self, name: str) -> _Task:
        return self.tasks_.get(name, None)

    def exit(self):
        with self.lock_:
            self.want_exit_ = True
            os.write(self.write_fd_, b"\0")

    def run(self):
        while True:
            # Sleep until the top of the next minute (+1 sec so jitter doesn't make us run the same minute twice)
            t = datetime.now()
            interval = 60 - t.second - t.microsecond / 1000000.0 + 1
            readable, _, _ = select.select([self.read_fd_], [], [], interval)
            if readable:
                os.read(self.read_fd_, 4096)

            # Check if we set the exit flag.
            with self.lock_:
                if self.want_exit_:
                    return

            # Get the new time and run any events that belong to this time.
            t = datetime.now()
            for task in self.tasks_.values():
                if task.should_run_at(t):
                    with self.lock_:
                        task.run()

