#include "DHT.h"
#include "Logging.h"

#include <stdio.h>
#include <unistd.h>

#include <bcm2835.h>

DHTType
TypeFromString(const char *name)
{
    if (0 == strcmp(name, "DHT11")) return DHT11;
    else if (0 == strcmp(name, "DHT22")) return DHT22;
    else if (0 == strcmp(name, "AM2302")) return AM2302;
    assert(false);
}

uint32_t
DHTReader::waitForState(bool state)
{
    uint32_t counter = 0;
    while (bcm2835_gpio_lev(pin_) != state && ++counter < timeoutCycles());
    return counter;
}

bool
DHTReader::readTimings()
{
    // Trigger the read-cycle by yanking on the wire in the agreed manner.
    bcm2835_gpio_fsel(pin_, BCM2835_GPIO_FSEL_OUTP);
    bcm2835_gpio_write(pin_, HIGH);
    usleep(500000);    // 500ms
    bcm2835_gpio_write(pin_, LOW);
    usleep(20000);    // 20ms

    // The DHT will pull high until it is ready, then pull low.
    bcm2835_gpio_fsel(pin_, BCM2835_GPIO_FSEL_INPT);
    uint32_t count = 0;
    const uint32_t timeout = 200000;
    while (bcm2835_gpio_lev(pin_) == true && ++count < timeout) // ~2s timeout.
        usleep(1);
    if (count == timeout) {
        fprintf(stderr, PRIO_ERR "Timed out waiting for DHT. Please double-check your pin settings.");
        return false;
    }

    // Discard the first bit.
    waitForState(true);
    waitForState(false);

    // Time each state transition. The timing here is extremely sensitive, so we
    // read the timings up front and worry about parsing the data later.
    for (uint32_t i = 0; i < NumTimings; ++i) {
        timings_[i] = waitForState(i % 2 == 0);
        if (timings_[i] == timeoutCycles()) {
            fprintf(stderr, PRIO_ERR "DHT timed out while reading.");
            return false;
        }
    }

    if (debug_) {
        for (uint32_t i = 0; i < NumTimings; ++i) {
            int expect = (i && i % 16 == 0) ? byteSyncDelay() : bitSyncDelay();
            if (i && i % 16 == 0)
                fprintf(stderr, PRIO_DEBUG "===");
            if (i % 2 == 0)
                fprintf(stderr, PRIO_DEBUG "sync: %d: %d", timings_[i], (int)timings_[i] - expect);
            else
                fprintf(stderr, PRIO_DEBUG "bit : %d ----> %d", timings_[i], timings_[i] > lowHighCutoff());
        }
    }

    return true;
}

bool
DHTReader::reconstructDataFromTimings()
{
    // Shift each bit into the output data register.
    for (uint32_t i = 1; i < NumTimings; i += 2) {
        bool bit = timings_[i] > lowHighCutoff();
        uint32_t bitOff = i / 2;
        uint32_t byteOff = bitOff / 8;
        data_[byteOff] = data_[byteOff] << 1 | bit;
    }

    if (debug_) {
        fprintf(stderr, PRIO_DEBUG "Data: 0x%x 0x%x 0x%x 0x%x: chkbyte 0x%x | chksum: 0x%x",
                   data_[0], data_[1], data_[2], data_[3], data_[4],
                   (data_[0] + data_[1] + data_[2] + data_[3]) & 0xFF);
    }

    return true;
}

bool
DHTReader::parseData()
{
    uint8_t checkByte = data_[4];
    uint8_t checkSum = (data_[0] + data_[1] + data_[2] + data_[3]) & 0xFF;
    if (checkByte != checkSum) {
        fprintf(stderr, PRIO_WARNING "Checksum mismatch! Got check byte: 0x%x, but checksum 0x%x",
                   checkByte, checkSum);
        return false;
    }

    if (type_ == DHT11) {
        temp_ = data_[2];
        humidity_ = data_[0];
        return true;
    }

    humidity_ = (data_[0] * 256 + data_[1]) / 10.0f;
    temp_ = ((data_[2] & 0x7F)* 256 + data_[3]) / 10.0f;
    if (data_[2] & 0x80)
        temp_ *= -1;

    if (temp_ < 0.0f || temp_ > 100.0f) {
        fprintf(stderr, PRIO_WARNING "Temperature data out of range, discarding: got %f", temp_);
        return false;
    }
    if (humidity_ < 0.0f || humidity_ > 100.0f) {
        fprintf(stderr, PRIO_WARNING "Humidity data out of range, discarding: got %f", humidity_);
        return false;
    }

    return true;
}

