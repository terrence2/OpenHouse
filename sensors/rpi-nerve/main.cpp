#include "DHT.h"
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

#include <libdaemon/daemon.h>

const static char *gShortOpts = "n:d:t:m:DWfkh";
const static struct option gLongOpts[] = {
    { "name", true, NULL, 'n' },
    { "dht-pin", true, NULL, 'd' },
    { "dht-type", true, NULL, 't' },
    { "motion-pin", true, NULL, 'm' },
    { "debug", false, NULL, 'D' },
    { "wait", false, NULL, 'W' },
    { "foreground", false, NULL, 'f' },
    { "kill", false, NULL, 'k' },
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
    "  -W,--wait       Wait in the child for gdb attach.\n"
    "  -f,--foreground Do not fork into background.\n"
    "  -k,--kill       Kill the running daemon.\n"
    "  -h,--help       Print this help message.\n";

struct Options
{
    char *name;
    DHTType dhtType;
    uint16_t dhtPin;
    uint16_t motionPin;
    bool haveName;
    bool haveDHTPin;
    bool haveDHTType;
    bool haveMotionPin;
    bool debugMode;
    bool waitMode;
    bool killMode;
    bool foregroundMode;

    Options()
      : name(NULL),
        dhtType(DHTType(-1)),
        dhtPin(uint16_t(-1)),
        motionPin(uint16_t(-1)),
        haveName(false),
        haveDHTPin(false),
        haveDHTType(false),
        haveMotionPin(false),
        debugMode(false),
        waitMode(false),
        killMode(false),
        foregroundMode(false)
    {}

    /* Returns true if the program should end. */
    int parse(int argc, char **argv) {
        int ret;
        while ((ret = getopt_long(argc, argv, gShortOpts, gLongOpts, NULL)) >= 0) {
            char optchar = (char)ret;
            switch (optchar) {
            case 'n': haveName = true; name = optarg; break;
            case 'd': haveDHTPin = true; dhtPin = atoi(optarg); break;
            case 't': haveDHTType = true; dhtType = TypeFromString(optarg); break;
            case 'm': haveMotionPin = true; motionPin = atoi(optarg); break;
            case 'D': debugMode = true; break;
            case 'W': waitMode = true; break;
            case 'k': killMode = true; break;
            case 'f': foregroundMode = true; break;
            case 'h': printf(gHelpText); return true;
            default: assert(false);
            }
        }
        return false;
    }
};

static int mainloop(const Options &opts);
static bool got_exit_signal();

int main(int argc, char **argv)
{
    daemon_pid_file_ident = daemon_log_ident = daemon_ident_from_argv0(argv[0]);

    Options opts;
    if (opts.parse(argc, argv))
        return 0;

    if (opts.debugMode)
        daemon_set_verbosity(LOG_DEBUG);

    daemon_log_use = DAEMON_LOG_AUTO;
    if (opts.foregroundMode)
        daemon_log_use = DAEMON_LOG_STDERR;

    if (opts.killMode) {
        int ret = daemon_pid_file_kill_wait(SIGTERM, 5);
        if (ret < 0)
            daemon_log(LOG_WARNING, "Failed to kill running daemon: %s", strerror(errno));
        return ret < 0 ? 1 : 0;
    }

    if (!opts.haveName) {
        daemon_log(LOG_ERR, "A name is requried to connect to the MCP.");
        return 1;
    }
    if (!opts.haveDHTPin) {
        daemon_log(LOG_ERR, "The pin# the dht device is connected to is required.");
        return 1;
    }
    if (!opts.haveDHTType) {
        daemon_log(LOG_ERR, "The specific device type must be provided, one of: DHT11, DHT22, AM2302.");
        return 1;
    }
    if (!opts.haveMotionPin) {
        daemon_log(LOG_ERR, "The pin# the motion device is connected to is required.");
        return 1;
    }

    if (daemon_reset_sigs(-1) < 0) {
        daemon_log(LOG_ERR, "Failed to reset signals: %s", strerror(errno));
        return 1;
    }

    if (daemon_unblock_sigs(-1) < 0 ) {
        daemon_log(LOG_ERR, "Failed to unblock signals: %s", strerror(errno));
        return 1;
    }

    pid_t pid = daemon_pid_file_is_running();
    if (pid >= 0) {
        daemon_log(LOG_ERR, "The daemon is already running on pid: %d", pid);
        return 1;
    }

    if (daemon_retval_init() < 0) {
        daemon_log(LOG_ERR, "Failed to create daemon pipe: %s", strerror(errno));
        return 1;
    }

    if (!opts.foregroundMode) {
        pid = daemon_fork();
        if (pid < 0) {
            daemon_log(LOG_ERR, "Failed to fork into daemon mode: %s", strerror(errno));
            daemon_retval_done();
            return 1;
        }

        // Parent
        if (pid) {
            if (opts.waitMode)
                fprintf(stderr, "Child PID: %d\n", pid);
            int ret = daemon_retval_wait(60);
            if (ret < 0) {
                daemon_log(LOG_ERR, "Timed out waiting for daemon to initialize!");
                return 255;
            }
            return ret;
        }
    }

    // Child
    int rv = mainloop(opts);
    daemon_log(LOG_INFO, "Finished running");
    // If we haven't sent yet, send error, otherwise the parent will ignore the second send.
    daemon_retval_send(255);
    daemon_signal_done();
    daemon_pid_file_remove();
    return rv;
}

static int
mainloop(const Options &opts)
{
    while (opts.waitMode)
        sleep(10);

    if (daemon_close_all(-1) < 0) {
        daemon_log(LOG_ERR, "Failed to close all descriptors.");
        return 1;
    }

    if (daemon_pid_file_create() < 0) {
        daemon_log(LOG_ERR, "Failed to create the daemon's pid file.");
        return 1;
    }

    if (daemon_signal_init(SIGINT, SIGTERM, SIGQUIT, SIGHUP, 0) < 0) {
        daemon_log(LOG_ERR, "Could not register signal handlers (%s).", strerror(errno));
        return 1;
    }

    if (!bcm2835_init()) {
        daemon_log(LOG_ERR, "Failed to initialize broadcom 2835 device. Are we running as root?");
        return 1;
    }

    Network net(opts.name);
    DHTReader dht(opts.dhtType, opts.dhtPin, opts.debugMode);
    MotionDetector motion(opts.motionPin);

    // Unblock the parent once we are done with initialization and are sure we can continue.
    if (!opts.foregroundMode)
        daemon_retval_send(0);
    daemon_log(LOG_INFO, "Nerve initialized.");

    while (!got_exit_signal()) {
        if (dht.read()) {
            net.updateTempAndHumidity(dht.celsius(), dht.humidity());
            daemon_log(LOG_DEBUG, "Temp = %.1f *C, %.1f *F, Hum = %.1f%%\n",
                       dht.celsius(), dht.fahrenheit(), dht.humidity());
        }

        time_t next = time(NULL) + 3;
        while (time(NULL) < next) {
            if (motion.waitForMotion(3000000))
                net.detectedMovement(motion.state());
            daemon_log(LOG_DEBUG, "MotionState: %d\n", motion.state());
        }
    }

    return 0;
}

static bool
got_exit_signal()
{
    int signalfd = daemon_signal_fd();
    fd_set fds;
    FD_ZERO(&fds);
    FD_SET(signalfd, &fds);
    struct timeval tv = { 0, 0 };
    if (select(FD_SETSIZE, &fds, 0, 0, &tv) < 0) {
        if (errno == EINTR)
            return false;

        daemon_log(LOG_ERR, "select(): %s", strerror(errno));
        return true;
    }

    if (FD_ISSET(signalfd, &fds)) {
        int sig = daemon_signal_next();
        if (sig <= 0) {
            daemon_log(LOG_ERR, "daemon_signal_next() failed: %s", strerror(errno));
            return false;
        }

        switch (sig) {
        case SIGINT:
        case SIGQUIT:
        case SIGTERM:
            daemon_log(LOG_WARNING, "Got SIGINT, SIGQUIT or SIGTERM.");
            return true;
        default:
            return false;
        }
    }
    return false;
}

