#ifndef DHT_h__
#define DHT_h__

#include <assert.h>
#include <string.h>
#include <stdint.h>

enum DHTType {
    DHT11,
    DHT22,
    AM2302
};
DHTType TypeFromString(const char *name);

// Wraps the code necessary to read the temperature and humidity from a DHT device.
class DHTReader 
{
    const static uint32_t NumTimings = 80; // 5 bytes * 8 bits / byte * 2 transitions / bit
    const static uint32_t TimeoutCycles = 100000; // How many cycles before we give up.

    const DHTType type_;
    const uint8_t pin_;

    // Print out lots of extra debugging information about timings. Useful for
    // setting the clock scale appropriately.
    bool debug_;

    // The DHT holds the wire high or low for very little time when writing out
    // its data. It is a small enough window that on a raspberry pi we don't have
    // time to take a syscall to get the current time. Thus, we use the time it
    // takes to read the bit itself as a proxy for real time and count
    // everything in terms of these cycles.
    //
    // On a stock clocked raspberry pi model B, with debian's build of g++ 4.6.3,
    // compiled with optimization level -O3, this comes out to:
    //    ~ 200 cycles for the sync pulse between bits
    //    ~ 250 cycles for the sync pulse between bytes
    //    ~  95 cycles for low bits
    //    ~ 265 cycles for high bits
    //
    // If you've overclocked your pi, if you are using a different compiler, if
    // the broadcom chipset driver gets faster, if you've slathered your pi in
    // honey, or whatever else, you can use the clock scale to adjust the above
    // timings to get a more reliable read. You should be able to use
    // --print-timings, this comment, and some arithmatic, to get the right
    // --scale for your environment.
    const float clockScale_;

    uint32_t timeoutCycles() const { return TimeoutCycles * clockScale_; }
    int bitSyncDelay() const { return 200 * clockScale_; }
    int byteSyncDelay() const { return 250 * clockScale_; }
    int lowHighCutoff() const { return 180 * clockScale_; }

    // State data.
    uint16_t timings_[NumTimings];
    uint8_t data_[5];
    bool lastState_;
    float temp_;
    float humidity_;

    // Reset internal state for next read.
    bool reset() {
        memset(timings_, 0, sizeof(timings_));
        memset(data_, 0, sizeof(data_));
        lastState_ = true;
        temp_ = 0.0f;
        humidity_ = 0.0f;
        return true;
    }

    uint32_t waitForState(bool state);
    bool readTimings();
    bool reconstructDataFromTimings();
    bool parseData();

    // Statistics. Total number of attempted reads and total read failures.
    uint32_t readCount_;
    uint32_t failureCount_;

  public:
    explicit DHTReader(DHTType type, uint8_t pin, bool debug = false, float clockScale = 1.0f)
      : type_(type), pin_(pin), debug_(debug), clockScale_(clockScale), temp_(0), humidity_(0),
        readCount_(0), failureCount_(0)
    {}

    bool read() {
        ++readCount_;
        bool success = reset() &&
                       readTimings() &&
                       reconstructDataFromTimings() &&
                       parseData();
        if (!success)
            failureCount_++;
        return success;
    }

    float celsius() const {
        assert(temp_ > 0.0f);
        return temp_;
    }

    float fahrenheit() const {
        return celsius() * 9.0f / 5.0f + 32.0f;
    }

    float humidity() const {
        assert(humidity_ > 0.0f);
        return humidity_;
    }

    float failureRate() const {
        return float(failureCount_) / float(readCount_) * 100.0f;
    }
};

#endif // DHT_h__
