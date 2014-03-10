#include "DHT.h"
#include "Logging.h"
#include "MotionDetector.h"
#include "Network.h"

#include <getopt.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/select.h>
#include <sys/time.h>
#include <sys/types.h>
#include <unistd.h>

#include <bcm2835.h>

const static char *gShortOpts = "n:d:t:m:Dh";
const static struct option gLongOpts[] = {
    { "name", false, NULL, 'n' },
    { "dht-pin", true, NULL, 'd' },
    { "dht-type", true, NULL, 't' },
    { "motion-pin", true, NULL, 'm' },
    { "debug", false, NULL, 'D' },
    { "help", false, NULL, 'h' },
    { NULL, 0, NULL, 0 }
};
const static char *gHelpText =
    "Usage: nerve -n NAME -d PIN -t TYPE -m PIN\n"
    "  -n,--name       The name to connect as.\n"
    "  -d,--dht-pin    The pin the DHT is on.\n"
    "  -t,--dht-type   The type of DHT.\n"
    "  -m,--motion-pin The pin the motion detector is on.\n"
    "  -D,--debug      Log extra debugging info.\n"
    "  -h,--help       Print this help message.\n";

struct Options
{
    char *name;
    DHTType dhtType;
    uint16_t dhtPin;
    uint16_t motionPin;
    bool haveDHTPin;
    bool haveDHTType;
    bool haveMotionPin;
    bool debugMode;

    Options()
      : name(NULL),
        dhtType(DHTType(-1)),
        dhtPin(uint16_t(-1)),
        motionPin(uint16_t(-1)),
        haveDHTPin(false),
        haveDHTType(false),
        haveMotionPin(false),
        debugMode(false)
    {
        const size_t maxlen = 256;
        char defname[maxlen];
        int rv = gethostname(defname, maxlen - 1);
        assert(rv == 0);
        name = strdup(defname);
    }

    ~Options() {
        free(name);
    }

    /* Returns true if the program should end. */
    int parse(int argc, char **argv) {
        int ret;
        while ((ret = getopt_long(argc, argv, gShortOpts, gLongOpts, NULL)) >= 0) {
            char optchar = (char)ret;
            switch (optchar) {
            case 'n': free(name); name = strdup(optarg); break;
            case 'd': haveDHTPin = true; dhtPin = atoi(optarg); break;
            case 't': haveDHTType = true; dhtType = TypeFromString(optarg); break;
            case 'm': haveMotionPin = true; motionPin = atoi(optarg); break;
            case 'D': debugMode = true; break;
            case 'h': printf(gHelpText); return true;
            default: assert(false);
            }
        }
        return false;
    }
};

static int mainloop(const Options &opts);

static bool gExitRequested = false;

void
sigterm_callback(int)
{
    gExitRequested = true;
}

void
sighup_callback(int)
{
}

int
main(int argc, char **argv)
{
    Options opts;
    if (opts.parse(argc, argv))
        return 0;

    if (!opts.haveDHTPin) {
        fprintf(stderr, PRIO_ERR "The pin# the dht device is connected to is required.\n");
        return 1;
    }
    if (!opts.haveDHTType) {
        fprintf(stderr, PRIO_ERR "The specific device type must be provided, one of: DHT11, DHT22, AM2302.\n");
        return 1;
    }
    if (!opts.haveMotionPin) {
        fprintf(stderr, PRIO_ERR "The pin# the motion device is connected to is required.\n");
        return 1;
    }

    int rv = mainloop(opts);
    fprintf(stderr, PRIO_INFO "Finished running: %d\n", rv);
    return rv;
}

static int
mainloop(const Options &opts)
{
    if (!bcm2835_init()) {
        fprintf(stderr, PRIO_ERR "Failed to initialize broadcom 2835 device. Are we running as root?\n");
        return 1;
    }

    Network net(opts.name);
    DHTReader dht(opts.dhtType, opts.dhtPin, opts.debugMode);
    MotionDetector motion(opts.motionPin);

    signal(SIGTERM, sigterm_callback);
    signal(SIGHUP, sighup_callback);

    fprintf(stderr, PRIO_INFO "Nerve %s initialized.\n", opts.name);

    while (!gExitRequested) {
        if (dht.read()) {
            net.updateTempAndHumidity(dht.celsius(), dht.humidity());
            fprintf(stderr, PRIO_INFO "Motion: %d, Temp = %.1f *C (%.1f *F), Hum = %.1f%% [%.2f%% failure rate]\n",
                    motion.state(), dht.celsius(), dht.fahrenheit(), dht.humidity(),
                    dht.failureRate());
        }

        time_t next = time(NULL) + 3;
        while (time(NULL) < next) {
            if (motion.waitForMotion(3000000))
                net.detectedMovement(motion.state());
        }
    }

    return 0;
}

