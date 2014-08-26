import time

from datetime import datetime, timedelta
from unittest import TestCase
from threading import Lock

from mcp.scheduler import Scheduler


class TestScheduler(TestCase):
    def test_quit(self):
        scheduler = Scheduler(Lock())
        scheduler.start()
        scheduler.exit()
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

        scheduler.exit()
        scheduler.join()

    def test_negative_update(self):
        scheduler = Scheduler(Lock())
        scheduler.start()

        called = False
        def callback():
            nonlocal called
            called = True
        scheduler.set_timeout(timedelta(seconds=-10), callback)
        time.sleep(0.5)
        self.assertEqual(called, True)

        scheduler.exit()
        scheduler.join()
