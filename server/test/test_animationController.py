# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from contextlib import contextmanager
from threading import Lock
from time import sleep
from unittest import TestCase

from mcp.animation import AnimationController, CallbackAnimation, LinearAnimation


@contextmanager
def Animator(delay=0.1):
    animator = AnimationController(delay, Lock())
    animator.start()
    yield animator
    animator.exit()
    animator.join()


class TestAnimationController(TestCase):

    def test_animate_callback(self):
        with Animator() as animator:
            count = 0
            def callback1():
                nonlocal count
                count += 1
            animator.animate(CallbackAnimation(callback1))
            sleep(1)

        self.assertTrue(7 < count < 13)

    def test_cancel_ongoing_animation(self):
        with Animator() as animator:
            count = 0
            def callback1():
                nonlocal count
                count += 1
            animator.animate(CallbackAnimation(callback1))

            sleep(1)
            animator.cancel_ongoing_animation()
            sleep(1)

        self.assertTrue(7 < count < 13)

    def test_linear_animation(self):
        with Animator() as animator:
            last = -1
            finished = False

            def tick(v: float):
                nonlocal last
                self.assertGreater(v, last)
                last = v

            def finish():
                nonlocal finished
                finished = True

            animator.animate(LinearAnimation(0, 1, 1, tick, finish))
            sleep(1.5)
            self.assertEqual(last, 1)
            self.assertTrue(finished)

