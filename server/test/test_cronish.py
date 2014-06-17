import tempfile
import threading
import time

from datetime import datetime
from unittest import TestCase

from mcp.cronish import Cronish


class TestCronish(TestCase):
    def test_register_task(self):
        cronish = Cronish(tempfile.gettempdir(), threading.Lock())

        def call_foo():
            pass
        cronish.register_task('foo', call_foo)

    def test_update_task_time(self):
        cronish = Cronish(tempfile.gettempdir(), threading.Lock())
        self.assertRaises(KeyError, lambda: cronish.update_task_time('foo', days_of_week={0}, hours={0}, minutes={0}))
        def call_foo():
            pass
        cronish.register_task('foo', call_foo)
        cronish.update_task_time('foo', days_of_week={0}, hours={0}, minutes={0})

    def test_run(self):
        cronish = Cronish(tempfile.gettempdir(), threading.Lock())
        cronish.start()
        cronish.exit()
        cronish.join()

    def test_save_load(self):
        cronish = Cronish('', threading.Lock())
        def call_foo():
            pass
        cronish.register_task('foo', call_foo)
        cronish.update_task_time('foo', days_of_week={0}, hours={0}, minutes={0})

    def _test_calls_task_once(self):
        """Disabled by default as can take up to a minute."""
        cronish = Cronish(tempfile.gettempdir(), threading.Lock())

        foo_calls = 0
        def call_foo():
            nonlocal foo_calls
            foo_calls += 1
        now = datetime.now()
        cronish.register_task('foo', call_foo)
        cronish.update_task_time('foo', days_of_week={now.weekday()}, hours={now.hour}, minutes={(now.minute + 1) % 60})

        cronish.start()
        time.sleep(60)
        cronish.exit()
        cronish.join()

        self.assertEqual(foo_calls, 1)
