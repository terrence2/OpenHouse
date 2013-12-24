#include "MotionDetector.h"

#include <sys/time.h>
#include <unistd.h>

#include <bcm2835.h>

MotionDetector::MotionDetector(const uint16_t pin)
  : pin_(pin), state_(false)
{
    bcm2835_gpio_fsel(pin_, BCM2835_GPIO_FSEL_INPT);
}

bool
MotionDetector::waitForMotion(const suseconds_t usec)
{
    struct timeval initial, cur, rv;
    gettimeofday(&initial, NULL);

    suseconds_t total;
    do {
        bool newState = bcm2835_gpio_lev(pin_);
        if (newState != state_) {
            state_ = newState;
            return true;
        }

        // Try to check about 10x a second, really more like 5 with overhead.
        usleep(100000);

        // Check for loop end.
        gettimeofday(&cur, NULL);
        timersub(&cur, &initial, &rv);
        total = rv.tv_sec * 1000000 + rv.tv_usec;
    } while (total < usec);

    return false;
}

bool
MotionDetector::state()
{
    return state_;
}

