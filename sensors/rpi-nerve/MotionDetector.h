/* This Source Code Form is subject to the terms of the GNU General Public
 * License, version 3. If a copy of the GPL was not distributed with this file,
 * You can obtain one at https://www.gnu.org/licenses/gpl.txt. */
#ifndef MotionDetector_h__
#define MotionDetector_h__

#include <stdint.h>
#include <sys/time.h>

class MotionDetector
{
    uint16_t pin_;
    bool state_;

  public:
    MotionDetector(const uint16_t pin);

    /*
     * Poll until we detect the presence of motion or until
     * msec milliseconds pass. Returns true if a state change
     * happened.
     */
    bool waitForMotion(const suseconds_t usec);

    /* Read the current device state. */
    bool state();
};

#endif // MotionDetector_h__
