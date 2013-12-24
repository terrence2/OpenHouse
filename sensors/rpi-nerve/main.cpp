#include "DHT.h"
#include "MotionDetector.h"
#include "Network.h"

#include <getopt.h>
#include <stdio.h>
#include <stdlib.h>

#include <bcm2835.h>

/*
 * rpi-nerve-bedroom:
 *     dht-pin: 4
 *     dht-type: AM2302
 *     motion-pin: 23
 *     led-pin: 17
 */

const char *shortopts = "n:d:t:m:h";
const struct option longopts[] = {
    { "name", true, NULL, 'n' },
    { "dht-pin", true, NULL, 'd' },
    { "dht-type", true, NULL, 't' },
    { "motion-pin", true, NULL, 'm' },
    { "help", false, NULL, 'h' },
    { NULL, 0, NULL, 0 }
};

int main(int argc, char **argv)
{
    bool haveName = false;
    const char *name = NULL;
    bool haveDHTPin = false;
    uint16_t dhtPin = (uint16_t)-1;
    bool haveDHTType = false;
    DHTType dhtType = (DHTType)-1;
    bool haveMotionPin = false;
    uint16_t motionPin = (uint16_t)-1;
    int goret;
    while ((goret = getopt_long(argc, argv, shortopts, longopts, NULL)) >= 0) {
        char optchar = (char)goret;
        switch (optchar) {
        case 'n': haveName = true; name = optarg; break;
        case 'd': haveDHTPin = true; dhtPin = atoi(optarg); break;
        case 't': haveDHTType = true; dhtType = TypeFromString(optarg); break;
        case 'm': haveMotionPin = true; motionPin = atoi(optarg); break;
        case 'h': printf("Usage: nerve -n NAME -d PIN -t TYPE -m PIN\n"
                         "  -n,--name       The name to connect as.\n"
                         "  -d,--dht-pin    The pin the DHT is on.\n"
                         "  -t,--dht-type   The type of DHT.\n"
                         "  -m,--motion-pin The pin the motion detector is on.\n"
                         "  -h,--help       Print this help message.\n");
                  return 0;
        default: assert(false);
        }
    }

    if (!haveName) {
        printf("A name is requried to connect to the MCP.\n");
        return 1;
    }
    if (!haveDHTPin) {
        printf("The pin# the dht device is connected to is required.\n");
        return 1;
    }
    if (!haveDHTType) {
        printf("The specific device type must be provided, one of: DHT11, DHT22, AM2302.\n");
        return 1;
    }
    if (!haveMotionPin) {
        printf("The pin# the motion device is connected to is required.\n");
        return 1;
    }

    Network net(name);

    if (!bcm2835_init())
        return 1;

    DHTReader dht(dhtType, dhtPin);
    MotionDetector motion(motionPin);
    while (true) {
        if (dht.read()) {
            net.updateTempAndHumidity(dht.celsius(), dht.humidity());
            printf("Temp =    %.1f *C, %.1f *F, Hum = %.1f%%\n", dht.celsius(), dht.fahrenheit(), dht.humidity());
        }

        time_t next = time(NULL) + 3;
        while (time(NULL) < next) {
            if (motion.waitForMotion(3000000))
                net.detectedMovement();
            printf("MotionState: %d\n", motion.state());
        }
    }

    return 0;
}

