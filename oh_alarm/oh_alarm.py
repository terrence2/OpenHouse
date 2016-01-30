#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging
from apscheduler.executors.asyncio import AsyncIOExecutor
from apscheduler.jobstores.memory import MemoryJobStore
from apscheduler.schedulers.asyncio import AsyncIOScheduler
from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging
from oh_shared.home import Home, NodeData

log = logging.getLogger('oh_alarm')

CRON_KEYS = ('year', 'month', 'day', 'week', 'day_of_week', 'hour', 'minute', 'second')


@asyncio.coroutine
def on_alarm(home: Home, path: str):
    alarms = yield from home.query(home.path_to_query(path)).run()
    alarm = alarms[path]
    log.info("alarm {} activated; switching to scene {}".format(path, alarm.attrs['scene']))
    yield from home.query('home').attr('scene', alarm.attrs['scene']).run()


def on_alarm_callback(loop: asyncio.BaseEventLoop, home: Home, path: str):
    """
    This is called by apschedule off the main thread. We need to trampoline back onto the
    main thread so that we can safely schedule the coroutine.
    """
    def _trampoline(home: Home, path: str):
        assert loop == asyncio.get_event_loop()
        loop.create_task(on_alarm(home, path))
    loop.call_soon_threadsafe(_trampoline, home, path)


@asyncio.coroutine
def main():
    args = parse_default_args('Do things at times.')
    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))

    scheduler = AsyncIOScheduler(jobstores={'default': MemoryJobStore()}, executors={'default': AsyncIOExecutor()},
                                 job_defaults={'coalesce': False})
    scheduler.start()

    alarms = yield from home.query("alarm").run()
    log.debug("Found {} jobs:".format(len(list(alarms.keys()))))
    for path, alarm in alarms.items():
        log.debug("\t{}: {}".format(path, alarm.attrs))
        cron_values = {key: alarm.attrs[key] for key in CRON_KEYS if key in alarm.attrs}
        scheduler.add_job(id=path, func=on_alarm_callback,
                          args=(asyncio.get_event_loop(), home, path),
                          trigger="cron", **cron_values)


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
