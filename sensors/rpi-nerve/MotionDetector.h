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
