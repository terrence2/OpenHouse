import time

from datetime import datetime, timedelta
from unittest import TestCase
from threading import Lock

from mcp.scheduler import Scheduler


class TestScheduler(TestCase):
    def test_quit(self):
        scheduler = Scheduler(Lock())
        scheduler.start()
        scheduler.quit()
        scheduler.join()

    def test_set_timeout(self):
        scheduler = Scheduler(Lock())
        scheduler.start()

        count = 0
        def callback():
            nonlocal count
            count += 1
            if count < 6:
                scheduler.set_timeout(timedelta(milliseconds=500), callback)

        scheduler.set_timeout(timedelta(milliseconds=500), callback)

        time.sleep(3.5)
        self.assertEqual(count, 6)

        scheduler.quit()
        scheduler.join()
