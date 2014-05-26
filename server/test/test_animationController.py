# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from threading import Lock
from time import sleep
from unittest import TestCase

from mcp.animation import AnimationController, CallbackAnimation


class TestAnimationController(TestCase):
    def test_run(self):
        animator = AnimationController(0.1, Lock())
        animator.start()
        animator.exit()
        animator.join()

    def test_animate(self):
        animator = AnimationController(0.1, Lock())
        animator.start()

        count = 0
        def callback1():
            nonlocal count
            count += 1
        animator.animate(CallbackAnimation(callback1))

        sleep(1)

        animator.exit()
        animator.join()

        self.assertTrue(7 < count < 13)

    def test_cancel_ongoing_animation(self):
        animator = AnimationController(0.1, Lock())
        animator.start()

        count = 0
        def callback1():
            nonlocal count
            count += 1
        animator.animate(CallbackAnimation(callback1))

        sleep(1)
        animator.cancel_ongoing_animation()
        sleep(1)

        animator.exit()
        animator.join()

        self.assertTrue(7 < count < 13)
